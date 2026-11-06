//! Turning a post into another language.
//!
//! Apple's Translation framework (`TranslationSession`) is Swift-only and
//! needs iOS 18 or macOS 15. This NSH 0.3.0 package has no Swift host, so it
//! cannot call that API. The working path is an OpenAI-compatible
//! `POST {base}/chat/completions` the user configures. No API key is built in.

use serde_json::Value;

pub const NO_ENDPOINT: &str = "还没有配置翻译。打开设置，填写接口地址和 API Key。本机的 Apple 翻译要 iOS 18 或 macOS 15，而且是 Swift 接口，这个包接不进去。";
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub ok: bool,
    pub text: String,
    pub error: String,
    pub cached: bool,
}

impl Outcome {
    pub fn json(&self) -> String {
        serde_json::json!({
            "ok": self.ok,
            "text": self.text,
            "error": self.error,
            "cached": self.cached,
        })
        .to_string()
    }

    fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            text: String::new(),
            error: message.into(),
            cached: false,
        }
    }

    fn hit(text: String, cached: bool) -> Self {
        Self {
            ok: true,
            text,
            error: String::new(),
            cached,
        }
    }
}

/// Remembered translations, oldest first. Capped by the caller.
#[derive(Debug, Default, Clone)]
pub struct Memory {
    pub entries: Vec<(String, String)>,
}

impl Memory {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(have, _)| have == key)
            .map(|(_, text)| text.as_str())
    }

    pub fn insert(&mut self, key: String, text: String, cap: usize) {
        self.entries.retain(|(have, _)| have != &key);
        self.entries.push((key, text));
        if self.entries.len() > cap {
            let extra = self.entries.len() - cap;
            self.entries.drain(0..extra);
        }
    }
}

/// `zh`, `en`, `ja`, or `system`.
pub fn canon_lang(raw: &str) -> Result<String, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "zh" | "zh-hans" | "zh-cn" | "chinese" => Ok("zh".into()),
        "en" | "en-us" | "english" => Ok("en".into()),
        "ja" | "ja-jp" | "japanese" => Ok("ja".into()),
        "system" | "" => Ok("system".into()),
        _ => Err("翻译语言只能是简体中文、English、日本語，或跟随系统。".into()),
    }
}

/// The language code sent to the endpoint.
pub fn target_code(lang: &str, locale: &str) -> String {
    let lang = canon_lang(lang).unwrap_or_else(|_| "zh".into());
    if lang != "system" {
        return lang;
    }
    let locale = locale.trim().to_ascii_lowercase();
    if locale.starts_with("zh") {
        "zh".into()
    } else if locale.starts_with("ja") {
        "ja".into()
    } else if locale.starts_with("en") {
        "en".into()
    } else if locale.is_empty() {
        "zh".into()
    } else {
        "en".into()
    }
}

/// Drops a trailing slash and a pasted `/chat/completions` suffix.
pub fn normalize_base(url: &str) -> String {
    let mut base = url.trim().trim_end_matches('/').to_string();
    let suffix = "/chat/completions";
    if base.ends_with(suffix) {
        base.truncate(base.len() - suffix.len());
        base = base.trim_end_matches('/').to_string();
    }
    base
}

pub fn completions_url(base: &str) -> String {
    format!("{}/chat/completions", normalize_base(base))
}

pub fn canon_model(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(DEFAULT_MODEL.into());
    }
    let ok = raw.len() <= 80
        && raw
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_' | '.' | ':' | '/'));
    if ok {
        Ok(raw.to_string())
    } else {
        Err("模型名字只保留字母、数字、点、横线和斜线。".into())
    }
}

pub fn target_name(code: &str) -> &'static str {
    match code {
        "en" => "English",
        "ja" => "Japanese",
        _ => "Simplified Chinese",
    }
}

/// Chat Completions body. Low temperature, translation only, no preamble.
pub fn chat_request(model: &str, target: &str, text: &str) -> Value {
    let language = target_name(target);
    serde_json::json!({
        "model": model,
        "temperature": 0.2,
        "messages": [
            {
                "role": "system",
                "content": format!("You translate posts. Translate the user message into {language}. Return only the translation, with no preamble, notes, or quotation marks.")
            },
            { "role": "user", "content": text }
        ]
    })
}

pub fn endpoint_ok(url: &str) -> bool {
    let url = url.trim();
    if url.len() < 12 || url.len() > 300 || url.contains(' ') {
        return false;
    }
    if url.starts_with("https://") {
        return true;
    }
    url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost")
}

pub fn plain(title: &str, body: &str, quote: &str) -> String {
    let mut parts = Vec::new();
    for piece in [title.trim(), body.trim(), quote.trim()] {
        if piece.is_empty() {
            continue;
        }
        if parts.iter().any(|have: &String| have == piece) {
            continue;
        }
        parts.push(piece.to_string());
    }
    let mut text = parts.join("\n");
    if text.chars().count() > 4_000 {
        text = text.chars().take(4_000).collect();
    }
    text
}

pub fn cache_key(post_id: &str, target: &str, model: &str, text: &str) -> String {
    format!("{}|{target}|{model}|{}", post_id.trim(), fnv(text))
}

fn fnv(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

pub fn read_translation(body: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "翻译接口没有返回 JSON。".to_string())?;
    if let Some(message) = error_message(&value) {
        return Err(format!("翻译接口说：{}", clip(&message, 160)));
    }
    let text = value
        .pointer("/choices/0/message/content")
        .map(message_text)
        .unwrap_or_default();
    let text = text.trim();
    if text.is_empty() {
        return Err("翻译接口没有给出译文。".into());
    }
    Ok(clip(text, 6_000).to_string())
}

fn error_message(value: &Value) -> Option<String> {
    let error = value.get("error")?;
    if let Some(message) = error.as_str() {
        let message = message.trim();
        if !message.is_empty() {
            return Some(message.to_string());
        }
    }
    let message = error.get("message").and_then(|item| item.as_str()).unwrap_or("").trim();
    if message.is_empty() { None } else { Some(message.to_string()) }
}

fn message_text(content: &Value) -> String {
    if let Some(text) = content.as_str() {
        return text.to_string();
    }
    let Some(parts) = content.as_array() else {
        return String::new();
    };
    let mut out = String::new();
    for part in parts {
        if let Some(text) = part.as_str() {
            out.push_str(text);
        } else if let Some(text) = part.get("text").and_then(|item| item.as_str()) {
            out.push_str(text);
        }
    }
    out
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max).collect()
    }
}

/// Looks up the cache, or calls `fetch(completions_url, key, text, target, model)` once.
pub fn perform(
    lang: &str,
    locale: &str,
    url: &str,
    token: &str,
    model: &str,
    post_id: &str,
    title: &str,
    body: &str,
    quote: &str,
    memory: &mut Memory,
    fetch: impl FnOnce(&str, &str, &str, &str, &str) -> Result<String, String>,
) -> Outcome {
    let text = plain(title, body, quote);
    if text.trim().is_empty() {
        return Outcome::err("这篇没有可以翻译的文字。");
    }
    let target = target_code(lang, locale);
    let model = canon_model(model).unwrap_or_else(|_| DEFAULT_MODEL.into());
    let key = cache_key(post_id, &target, &model, &text);
    if let Some(hit) = memory.get(&key) {
        return Outcome::hit(hit.to_string(), true);
    }
    let url = normalize_base(url);
    if url.is_empty() || token.trim().is_empty() {
        return Outcome::err(NO_ENDPOINT);
    }
    if !endpoint_ok(&url) {
        return Outcome::err("接口地址要是 https，或本机的 http://127.0.0.1。");
    }
    match fetch(&completions_url(&url), token.trim(), &text, &target, &model) {
        Ok(raw) => match read_translation(&raw) {
            Ok(translated) => {
                memory.insert(key, translated.clone(), 200);
                Outcome::hit(translated, false)
            }
            Err(error) => Outcome::err(error),
        },
        Err(error) => Outcome::err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        chat_request, completions_url, endpoint_ok, perform, plain, read_translation, target_code, Memory,
        NO_ENDPOINT,
    };

    #[test]
    fn chinese_is_the_default_and_the_system_locale_can_override_it() {
        assert_eq!(target_code("zh", "en-US"), "zh");
        assert_eq!(target_code("system", "ja-JP"), "ja");
        assert_eq!(target_code("system", "en-GB"), "en");
        assert_eq!(target_code("system", ""), "zh");
        assert_eq!(target_code("nope", "ja"), "zh");
    }

    #[test]
    fn a_post_keeps_its_title_and_does_not_ask_twice() {
        let text = plain("外れたルール", "前回の記事では実験を書いた。", "");
        assert!(text.contains("外れたルール"));
        assert!(text.contains("前回の記事"));
        let mut memory = Memory::default();
        let mut calls = 0;
        let first = perform(
            "zh",
            "zh-Hans",
            "https://api.openai.com/v1/",
            "sk-test",
            "gpt-4o-mini",
            "post-1",
            "外れたルール",
            "前回の記事では実験を書いた。",
            "",
            &mut memory,
            |url, token, body, target, model| {
                calls += 1;
                assert_eq!(url, "https://api.openai.com/v1/chat/completions");
                assert_eq!(token, "sk-test");
                assert_eq!(target, "zh");
                assert_eq!(model, "gpt-4o-mini");
                assert!(body.contains("外れたルール"));
                Ok(r#"{"choices":[{"message":{"role":"assistant","content":"偏离的规则"}}]}"#.into())
            },
        );
        assert!(first.ok && !first.cached);
        assert_eq!(first.text, "偏离的规则");
        let second = perform(
            "zh",
            "",
            "https://api.openai.com/v1",
            "sk-test",
            "gpt-4o-mini",
            "post-1",
            "外れたルール",
            "前回の記事では実験を書いた。",
            "",
            &mut memory,
            |_url, _token, _body, _target, _model| {
                calls += 1;
                Ok(r#"{"choices":[{"message":{"content":"不该再请求"}}]}"#.into())
            },
        );
        assert!(second.cached);
        assert_eq!(second.text, "偏离的规则");
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_missing_endpoint_explains_the_floor_without_calling_out() {
        let mut memory = Memory::default();
        let outcome = perform("zh", "", "", "", "", "p", "", "hello", "", &mut memory, |_, _, _, _, _| {
            panic!("no request");
        });
        assert!(!outcome.ok);
        assert!(outcome.error.contains("设置"));
        assert_eq!(outcome.error, NO_ENDPOINT);
        let asked = chat_request("gpt-4o-mini", "zh", "外れたルール");
        assert_eq!(asked["model"], "gpt-4o-mini");
        assert_eq!(asked["temperature"], serde_json::json!(0.2));
        assert!(asked["messages"][0]["content"].as_str().unwrap().contains("Simplified Chinese"));
        assert_eq!(asked["messages"][1]["content"], "外れたルール");
        assert_eq!(
            completions_url("https://api.openai.com/v1/chat/completions/"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            read_translation(r#"{"choices":[{"message":{"content":"偏离的规则"}}]}"#).unwrap(),
            "偏离的规则"
        );
        assert!(read_translation(r#"{"error":{"message":"invalid api key"}}"#)
            .unwrap_err()
            .contains("invalid api key"));
        assert!(!endpoint_ok("http://example.com/v1"));
        assert!(endpoint_ok("http://127.0.0.1:8080/v1"));
    }
}
