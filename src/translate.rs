//! OpenAI-compatible chat translation plus a small on-disk cache.
//!
//! Apple Translation (`TranslationSession`) is not called here. It is a
//! Swift API on iOS 18 / macOS 15, and the NSH 0.3.0 host has no Swift
//! runtime to construct a session. Settings holds the API base, key, and model.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use crate::logic::translate::{self, Memory};

static CACHE: Mutex<Option<Memory>> = Mutex::new(None);

pub fn translate_post(post_id: &str, title: &str, body: &str, quote: &str, locale: &str) -> String {
    let (lang, url, token, model) = crate::store::translate_settings();
    let mut guard = CACHE.lock().unwrap_or_else(|poison| poison.into_inner());
    if guard.is_none() {
        *guard = Some(load_cache());
    }
    let memory = guard.as_mut().expect("cache");
    let before = memory.entries.len();
    let outcome = translate::perform(
        &lang,
        locale,
        &url,
        &token,
        &model,
        post_id,
        title,
        body,
        quote,
        memory,
        |url, token, text, target, model| post_json(url, token, &translate::chat_request(model, target, text)),
    );
    if outcome.ok && !outcome.cached && memory.entries.len() != before {
        save_cache(memory);
    }
    if outcome.ok {
        eprintln!(
            "[pulse] translate cached={} target_len={}",
            u8::from(outcome.cached),
            outcome.text.chars().count()
        );
    } else {
        eprintln!("[pulse] translate failed");
    }
    outcome.json()
}

pub fn complete(payload: serde_json::Value) -> Result<String, String> {
    let (_lang, url, token, _model) = crate::store::translate_settings();
    let url = translate::normalize_base(&url);
    if url.is_empty() || token.trim().is_empty() {
        return Err(translate::NO_ENDPOINT.into());
    }
    if !translate::endpoint_ok(&url) {
        return Err("接口地址要是 https，或本机的 http://127.0.0.1。".into());
    }
    post_json(&translate::completions_url(&url), token.trim(), &payload)
}

fn post_json(url: &str, token: &str, payload: &serde_json::Value) -> Result<String, String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .build();
    let agent: ureq::Agent = config.into();
    let bearer = format!("Bearer {token}");
    let mut response = agent
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Authorization", &bearer)
        .header("User-Agent", "Pulse/0.1")
        .send(payload.to_string())
        .map_err(|_| "翻译接口没有连上。检查地址和密钥后再试。".to_string())?;
    response
        .body_mut()
        .read_to_string()
        .map_err(|_| "翻译接口的回复读不完。".to_string())
}

fn load_cache() -> Memory {
    let Some(path) = cache_path() else {
        return Memory::default();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return Memory::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Memory::default();
    };
    let Some(rows) = value.get("entries").and_then(|item| item.as_array()) else {
        return Memory::default();
    };
    let mut memory = Memory::default();
    for row in rows {
        let key = row.get("key").and_then(|item| item.as_str()).unwrap_or("");
        let translated = row.get("text").and_then(|item| item.as_str()).unwrap_or("");
        if !key.is_empty() && !translated.is_empty() {
            memory.insert(key.to_string(), translated.to_string(), 200);
        }
    }
    memory
}

fn save_cache(memory: &Memory) {
    let Some(path) = cache_path() else {
        return;
    };
    let entries: Vec<_> = memory
        .entries
        .iter()
        .map(|(key, text)| serde_json::json!({ "key": key, "text": text }))
        .collect();
    let body = serde_json::json!({ "entries": entries }).to_string();
    if let Err(error) = fs::write(path, body) {
        eprintln!("[pulse] translate cache {error}");
    }
}

fn cache_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut dir = PathBuf::from(home);
    dir.push("Library");
    dir.push("Application Support");
    dir.push(crate::legacy::SUPPORT_DIR);
    fs::create_dir_all(&dir).ok()?;
    dir.push("translations.json");
    Some(dir)
}
