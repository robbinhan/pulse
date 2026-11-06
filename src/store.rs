//! The open dashboard, kept in memory so a click does not parse it again.
//!
//! `view_slots` and `finish_fetch` used to send the whole profile, image URLs
//! included, through JSON on every call. The screen now asks for the small
//! answer, and a save serializes on another thread.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use crate::logic::{self, model::Profile};

struct Memory {
    profile: Arc<Profile>,
    revision: u64,
    /// `platform:index` -> (fingerprint, present JSON).
    present: HashMap<String, (u64, String)>,
    saving: bool,
    /// Revision last handed to the save thread.
    saving_revision: u64,
    /// Full profile JSON from the save thread, once it matches `revision`.
    export_json: String,
    export_revision: u64,
}

fn memory() -> &'static Mutex<Memory> {
    static MEMORY: OnceLock<Mutex<Memory>> = OnceLock::new();
    MEMORY.get_or_init(|| {
        Mutex::new(Memory {
            profile: Arc::new(Profile::default()),
            revision: 0,
            present: HashMap::new(),
            saving: false,
            saving_revision: 0,
            export_json: String::new(),
            export_revision: 0,
        })
    })
}

fn lock() -> std::sync::MutexGuard<'static, Memory> {
    memory().lock().unwrap_or_else(|poison| poison.into_inner())
}

pub fn install(profile: Profile) -> u64 {
    let mut guard = lock();
    guard.profile = Arc::new(profile);
    guard.revision = guard.revision.wrapping_add(1);
    guard.present.clear();
    guard.revision
}

fn touch(guard: &mut Memory) {
    guard.revision = guard.revision.wrapping_add(1);
}

/// Runs `change` on the live profile. Copies the profile only when a save
/// still holds the previous one.
fn write(change: impl FnOnce(&mut Profile)) -> u64 {
    let mut guard = lock();
    change(Arc::make_mut(&mut guard.profile));
    touch(&mut guard);
    guard.revision
}

fn read<T>(read: impl FnOnce(&Profile) -> T) -> T {
    let guard = lock();
    read(&guard.profile)
}

pub fn prepare_legacy() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let report = crate::legacy::migrate(Path::new(&home));
    if report.copied() {
        eprintln!(
            "[pulse] legacy copied profile={} storage={} webkit={}",
            report.profile as u8, report.storage as u8, report.webkit as u8
        );
    }
}

pub fn legacy_session() -> String {
    let Some(home) = std::env::var_os("HOME") else {
        return String::new();
    };
    crate::legacy::session_text(Path::new(&home)).unwrap_or_default()
}

static BOOTED: AtomicBool = AtomicBool::new(false);
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
static BG_EXPIRED: AtomicBool = AtomicBool::new(false);

pub fn boot(storage: &str) -> String {
    BOOTED.store(true, Ordering::SeqCst);
    #[cfg(target_os = "ios")]
    crate::refresh_ios::schedule();
    prepare_legacy();
    if let Some(text) = read_saved() {
        if let Ok(profile) = logic::model::normalize_json(&text) {
            install(profile);
            return dash(true);
        }
    }
    let recovered;
    let storage = if storage.trim().is_empty() {
        recovered = std::env::var_os("HOME")
            .and_then(|home| crate::legacy::profile_text(Path::new(&home)))
            .unwrap_or_default();
        recovered.as_str()
    } else {
        storage
    };
    if !storage.trim().is_empty() {
        if let Ok(profile) = logic::model::normalize_json(storage) {
            install(profile);
            queue_save();
            return dash(true);
        }
    }
    install(Profile::default());
    dash(true)
}

pub fn adopt_json(storage: &str) -> Result<String, String> {
    let profile = logic::model::normalize_json(storage)?;
    install(profile);
    queue_save();
    Ok(dash(true))
}

pub fn view_slots() -> String {
    read(|profile| to_json(&logic::model::focus_slots(profile)))
}

pub fn unread_totals() -> String {
    read(|profile| to_json(&logic::model::unread_totals(profile)))
}

pub fn focused_unread() -> String {
    read(|profile| logic::model::focused_unread(profile).to_string())
}

pub fn focused_fresh_ms() -> String {
    read(|profile| logic::model::focused_fresh_ms(profile).to_string())
}

pub fn card_clock(platform: &str, index: usize) -> String {
    read(|profile| to_json(&logic::model::card_clock(profile, platform, index)))
}

pub fn profile_url(platform: &str, index: usize) -> String {
    read(|profile| logic::model::profile_url_at(profile, platform, index))
}

pub fn present_card(platform: &str, index: usize) -> String {
    let mut guard = lock();
    let Some(card) = logic::model::card_at(&guard.profile, platform, index) else {
        return to_json(&empty_present("没有这个格子。"));
    };
    let fingerprint = fingerprint(card);
    let key = format!("{platform}:{index}");
    if let Some((have, json)) = guard.present.get(&key) {
        if *have == fingerprint {
            return json.clone();
        }
    }
    let rendered = match logic::model::present_card(&guard.profile, platform, index) {
        Ok(card) => to_json(&card),
        Err(error) => to_json(&empty_present(error)),
    };
    guard.present.insert(key, (fingerprint, rendered.clone()));
    rendered
}

pub fn finish_fetch(platform: &str, index: usize) -> String {
    let mut guard = lock();
    logic::model::finish_fetch(&mut Arc::make_mut(&mut guard.profile), platform, index);
    // The retry clock is for the scheduler. The bubbles did not change,
    // so the screen does not rebuild.
    r#"{"ok":true}"#.to_string()
}

pub fn ingest(platform: &str, index: usize, message: &str) -> String {
    if platform_rejected(platform) {
        return dash_err("没有这个平台。");
    }
    {
        let guard = lock();
        match logic::model::card_at(&guard.profile, platform, index) {
            None => return dash_err("没有这个格子。"),
            Some(card) if card.handle.is_empty() => return dash_err("这个格子还没有账号。"),
            Some(_) => {}
        }
    }
    if !message.trim_start().starts_with('{') {
        return dash_err("页面消息无法读取。");
    }
    let mut applied = false;
    let mut failed: Option<String> = None;
    write(|profile| {
        let owned = std::mem::take(profile);
        match logic::model::ingest(owned, platform, index, message) {
            Ok((next, flag)) => {
                applied = flag;
                *profile = next;
            }
            Err(error) => failed = Some(error),
        }
    });
    if let Some(error) = failed {
        return dash_err(&error);
    }
    if !applied {
        // `write` already counted a revision. Put it back so an ignored
        // page does not rebuild every card.
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
    }
    dash(applied)
}

pub fn next_fetches(now_ms: i64, busy: &[String], from: u32, until: u32) -> String {
    read(|profile| {
        let plan = logic::schedule::next_fetches(profile, now_ms, busy, from, until);
        to_json(&FetchReply {
            ok: true,
            error: String::new(),
            jobs: plan.jobs,
            max: plan.max,
        })
    })
}

pub fn set_home(home: &str) -> String {
    if logic::model::set_home(Profile::default(), home).is_err() {
        return dash_err("首页只能是动态或看板。");
    }
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::set_home(owned, home).expect("home was checked");
    });
    dash(true)
}

pub fn timeline(focus: &str, offset: u32, limit: u32, folder: &str) -> String {
    read(|profile| {
        to_json(&logic::model::timeline(
            profile,
            focus,
            offset as usize,
            limit as usize,
            folder,
        ))
    })
}

pub fn mark_post(platform: &str, index: usize, id: &str) -> String {
    let mut changed = false;
    write(|profile| {
        changed = logic::model::mark_post_read(profile, platform, index, id);
    });
    if !changed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash(false);
    }
    dash(true)
}

pub fn set_focus(focus: &str) -> String {
    if logic::model::set_focus(Profile::default(), focus).is_err() {
        return dash_err("没有这个平台。");
    }
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::set_focus(owned, focus).expect("focus was checked");
    });
    dash(true)
}

pub fn set_refresh(seconds: u32) -> String {
    if logic::model::set_refresh(Profile::default(), seconds).is_err() {
        return dash_err("刷新间隔要在 30 秒和 60 分钟之间。");
    }
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::set_refresh(owned, seconds).expect("refresh was checked");
    });
    dash(true)
}

pub fn translate_settings() -> (String, String, String, String) {
    read(|profile| {
        (
            profile.settings.translate_lang.clone(),
            profile.settings.translate_url.clone(),
            profile.settings.translate_token.clone(),
            profile.settings.translate_model.clone(),
        )
    })
}

pub fn set_translate(lang: &str, url: &str, token: &str, model: &str) -> String {
    if let Err(error) = logic::model::set_translate(Profile::default(), lang, url, token, model) {
        return dash_err(&error);
    }
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::set_translate(owned, lang, url, token, model).expect("translate was checked");
    });
    dash(true)
}

pub fn toggle_star(platform: &str, id: &str) -> String {
    let mut failed: Option<String> = None;
    write(|profile| match logic::model::toggle_star(profile, platform, id) {
        Ok(on) => eprintln!("[pulse] star platform={platform} on={}", u8::from(on)),
        Err(error) => failed = Some(error),
    });
    if let Some(error) = failed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    lock().present.clear();
    dash(true)
}

pub fn starred_timeline(offset: usize, limit: usize) -> String {
    read(|profile| to_json(&logic::model::starred_timeline(profile, offset, limit)))
}

pub fn set_briefing_hour(hour: u32) -> String {
    if hour > 23 {
        return dash_err("早报时间要在 0 点到 23 点之间。");
    }
    write(|profile| {
        profile.settings.briefing_hour = hour as u8;
    });
    dash(true)
}

pub fn briefing_status() -> String {
    read(|profile| {
        serde_json::json!({
            "ok": true,
            "day": profile.briefing.day,
            "script": profile.briefing.script,
            "note": profile.briefing.note,
            "count": profile.briefing.count,
            "hour": profile.settings.briefing_hour,
        })
        .to_string()
    })
}

pub fn set_briefing_auto(on: bool) -> String {
    write(|profile| {
        profile.settings.briefing_auto = on;
    });
    dash(true)
}

pub fn make_briefing(from_ms: i64, until_ms: i64, day: &str, force: bool) -> String {
    if !force && !read(|profile| profile.settings.briefing_auto) {
        return serde_json::json!({
            "ok": true,
            "empty": true,
            "script": "",
            "note": "",
            "count": 0,
        })
        .to_string();
    }
    let prepared = read(|profile| logic::briefing::prepare(profile, from_ms, until_ms, day, force));
    if !prepared.write {
        let note = if prepared.briefing.day == day.trim() {
            prepared.briefing.note.clone()
        } else {
            logic::briefing::setup_note().to_string()
        };
        return serde_json::json!({
            "ok": true,
            "empty": prepared.briefing.script.is_empty(),
            "script": prepared.briefing.script,
            "note": note,
            "count": prepared.briefing.count,
        })
        .to_string();
    }
    let mut briefing = prepared.briefing;
    if let Some(request) = prepared.request {
        match crate::translate::complete(request) {
            Ok(raw) => match logic::briefing::finish(briefing, &raw) {
                Ok(done) => briefing = done,
                Err(error) => {
                    eprintln!("[pulse] briefing failed");
                    return serde_json::json!({
                        "ok": false,
                        "empty": true,
                        "script": "",
                        "note": error,
                        "count": 0,
                    })
                    .to_string();
                }
            },
            Err(error) => {
                eprintln!("[pulse] briefing failed");
                return serde_json::json!({
                    "ok": false,
                    "empty": true,
                    "script": "",
                    "note": error,
                    "count": 0,
                })
                .to_string();
            }
        }
    }
    write(|profile| profile.briefing = briefing.clone());
    queue_save();
    eprintln!("[pulse] briefing day={} count={}", briefing.day, briefing.count);
    serde_json::json!({
        "ok": true,
        "empty": briefing.script.is_empty(),
        "script": briefing.script,
        "note": briefing.note,
        "count": briefing.count,
    })
    .to_string()
}

pub fn set_theme(theme: &str) -> String {
    if logic::model::set_theme(Profile::default(), theme).is_err() {
        return dash_err("外观只能是浅色、深色，或跟随系统。");
    }
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::set_theme(owned, theme).expect("theme was checked");
    });
    dash(true)
}

pub fn set_notify(on: bool) -> String {
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::set_notify(owned, on);
    });
    dash(true)
}

pub fn set_notify_platform(platform: &str, on: bool) -> String {
    if logic::model::set_notify_platform(Profile::default(), platform, on).is_err() {
        return dash_err("没有这个平台。");
    }
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::set_notify_platform(owned, platform, on).expect("platform was checked");
    });
    dash(true)
}

pub fn set_mute(raw: &str) -> String {
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::set_mute(owned, raw);
    });
    dash(true)
}

pub fn add_weibo(index: usize, uid: &str, label: &str, avatar: &str, query: &str, via: &str) -> String {
    let mut failed: Option<String> = None;
    write(|profile| {
        if let Err(error) = logic::model::add_weibo(profile, index, uid, label, avatar) {
            failed = Some(error);
        }
    });
    if let Some(error) = failed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    let query = one_line(query, 80);
    let via = one_line(via, 24);
    let uid = one_line(uid, 20);
    eprintln!("[pulse] weibo resolve q={query} uid={uid} via={via}");
    dash(true)
}

fn one_line(text: &str, max: usize) -> String {
    text.chars()
        .filter(|char| *char != '\n' && *char != '\r')
        .take(max)
        .collect()
}

pub fn ingest_rss(index: usize, xml: &str) -> String {
    let mut failed: Option<String> = None;
    let mut applied = false;
    write(|profile| match logic::model::ingest_rss(profile, index, xml) {
        Ok(flag) => applied = flag,
        Err(error) => failed = Some(error),
    });
    if let Some(error) = failed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    if !applied {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
    }
    dash(applied)
}

pub fn import_opml(xml: &str) -> String {
    import_opml_from(xml, "")
}

fn import_opml_from(xml: &str, path: &str) -> String {
    let mut failed: Option<String> = None;
    let mut added = 0u32;
    write(|profile| match logic::model::import_opml(profile, xml) {
        Ok(count) => added = count,
        Err(error) => failed = Some(error),
    });
    if let Some(error) = failed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    if path.is_empty() {
        eprintln!("[pulse] rss opml import added={added}");
    } else {
        eprintln!("[pulse] rss opml import file={path} added={added}");
    }
    dash(true)
}

pub fn export_opml() -> String {
    read(|profile| logic::model::export_opml(profile))
}

pub fn export_subscriptions() -> String {
    read(|profile| logic::sync::export_json(profile))
}

pub fn merge_subscriptions(json: &str) -> String {
    let mut changed = 0u32;
    let mut failed: Option<String> = None;
    write(|profile| match logic::sync::merge_json(profile, json) {
        Ok(count) => changed = count,
        Err(error) => failed = Some(error),
    });
    if let Some(error) = failed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    if changed == 0 {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash(false);
    }
    eprintln!("[pulse] sync merge changed={changed}");
    dash(true)
}

pub fn new_sync_token() -> String {
    logic::sync::new_token()
}

pub fn set_sync(mode: &str, url: &str, token: &str, pull_on_open: bool) -> String {
    write(|profile| logic::model::set_sync_prefs(profile, mode, url, token, pull_on_open));
    dash(true)
}

fn stamp_sync() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    write(|profile| profile.sync.last_sync_ms = now);
}

pub fn icloud_status() -> String {
    let report = crate::icloud::status();
    serde_json::json!({
        "available": report.available,
        "message": report.message,
    })
    .to_string()
}

pub fn icloud_pull() -> String {
    let json = match crate::icloud::read_list() {
        Ok(json) => json,
        Err(error) => return dash_err(&error),
    };
    let merged = merge_subscriptions(&json);
    if merged.contains("\"ok\":true") {
        stamp_sync();
    }
    // stamp_sync bumps revision after merge already returned a dash. Rebuild.
    if merged.contains("\"ok\":false") {
        return merged;
    }
    dash(true)
}

pub fn note_sync() -> String {
    stamp_sync();
    dash(true)
}

pub fn icloud_push() -> String {
    let json = export_subscriptions();
    if let Err(error) = crate::icloud::write_list(&json) {
        return dash_err(&error);
    }
    stamp_sync();
    eprintln!("[pulse] sync icloud push");
    dash(true)
}

pub fn rss_catalog() -> String {
    read(|profile| to_json(&logic::model::rss_catalog(profile)))
}

pub fn remove_rss(urls_json: &str) -> String {
    let urls: Vec<String> = serde_json::from_str(urls_json).unwrap_or_default();
    let mut removed = 0u32;
    write(|profile| {
        removed = logic::model::remove_rss_urls(profile, &urls);
    });
    if removed == 0 {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err("没有删掉订阅。");
    }
    eprintln!("[pulse] rss remove count={removed}");
    dash(true)
}

pub fn create_rss_folder(name: &str) -> String {
    let mut failed: Option<String> = None;
    write(|profile| {
        if let Err(error) = logic::model::create_rss_folder(profile, name) {
            failed = Some(error);
        }
    });
    if let Some(error) = failed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    eprintln!("[pulse] rss folder create");
    dash(true)
}

pub fn rename_rss_folder(id: &str, name: &str) -> String {
    let mut failed: Option<String> = None;
    write(|profile| {
        if let Err(error) = logic::model::rename_rss_folder(profile, id, name) {
            failed = Some(error);
        }
    });
    if let Some(error) = failed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    eprintln!("[pulse] rss folder rename id={id}");
    dash(true)
}

pub fn delete_rss_folder(id: &str) -> String {
    let mut failed: Option<String> = None;
    write(|profile| {
        if let Err(error) = logic::model::delete_rss_folder(profile, id) {
            failed = Some(error);
        }
    });
    if let Some(error) = failed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    eprintln!("[pulse] rss folder delete id={id}");
    dash(true)
}

pub fn move_card_folder(platform: &str, handle: &str, folder_id: &str) -> String {
    let mut failed: Option<String> = None;
    write(|profile| {
        if let Err(error) = logic::model::move_card_folder(profile, platform, handle, folder_id) {
            failed = Some(error);
        }
    });
    if let Some(error) = failed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    eprintln!("[pulse] folder move platform={platform}");
    dash(true)
}

pub fn move_rss(urls_json: &str, folder_id: &str) -> String {
    let urls: Vec<String> = serde_json::from_str(urls_json).unwrap_or_default();
    let mut failed: Option<String> = None;
    let mut moved = 0u32;
    write(|profile| match logic::model::move_rss_urls(profile, &urls, folder_id) {
        Ok(count) => moved = count,
        Err(error) => failed = Some(error),
    });
    if let Some(error) = failed {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    eprintln!("[pulse] rss move count={moved} folder={folder_id}");
    dash(true)
}

pub fn clear_empty_rss() -> String {
    let mut removed = 0u32;
    write(|profile| {
        removed = logic::model::clear_empty_rss(profile);
    });
    if removed == 0 {
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err("没有空订阅。");
    }
    eprintln!("[pulse] rss clear-empty count={removed}");
    dash(true)
}

pub fn import_opml_file() -> String {
    match crate::panels::pick_opml_text() {
        Ok(None) => crate::panels::reply_cancelled(),
        Err(error) => crate::panels::reply_error(&error),
        Ok(Some((path, xml))) => import_opml_from(&xml, &path),
    }
}

pub fn export_opml_file() -> String {
    let xml = export_opml();
    match crate::panels::save_opml_text(&xml) {
        Ok(None) => crate::panels::reply_cancelled(),
        Err(error) => crate::panels::reply_error(&error),
        Ok(Some(path)) => {
            eprintln!("[pulse] rss opml export file={path}");
            crate::panels::reply_saved(&path)
        }
    }
}

pub fn set_handle(platform: &str, index: usize, input: &str) -> String {
    let mut failed: Option<String> = None;
    write(|profile| {
        if let Err(error) = logic::model::set_handle_in(profile, platform, index, input) {
            failed = Some(error);
        }
    });
    if let Some(error) = failed {
        // The revision moved even though the handle did not. Step it back
        // so the screen does not rebuild for a rejected handle.
        let mut guard = lock();
        guard.revision = guard.revision.wrapping_sub(1);
        return dash_err(&error);
    }
    dash(true)
}

pub fn move_card(platform: &str, index: usize, direction: i32) -> String {
    if logic::model::move_card(Profile::default(), platform, index, direction).is_err() {
        return dash_err("没有这个平台。");
    }
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::move_card(owned, platform, index, direction).expect("platform was checked");
    });
    dash(true)
}

pub fn mark_read(platform: &str, index: usize) -> String {
    {
        let guard = lock();
        if logic::model::card_at(&guard.profile, platform, index).is_none() {
            return dash_err("没有这个格子。");
        }
    }
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::mark_read(owned, platform, index).expect("card was checked");
    });
    dash(true)
}

pub fn mark_all_read() -> String {
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::mark_all_read(owned);
    });
    dash(true)
}

pub fn request_refresh() -> String {
    write(|profile| {
        let owned = std::mem::take(profile);
        *profile = logic::model::request_refresh(owned);
    });
    dash(true)
}

pub fn sync_badge() {
    let total = read(|profile| logic::model::unread_totals(profile).all);
    crate::notify::set_badge(total);
}

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub fn begin_background_poll() {
    BG_EXPIRED.store(false, Ordering::SeqCst);
}

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub fn mark_background_expired() {
    BG_EXPIRED.store(true, Ordering::SeqCst);
}

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn background_expired() -> bool {
    BG_EXPIRED.load(Ordering::SeqCst)
}

/// RSS-only poll used by an iOS background wake. Loads the saved profile when
/// the UI has not booted yet, posts notices through the normal ingest path,
/// and writes the file before the process is suspended. Social accounts are
/// left for the foreground web views.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub fn background_poll() -> bool {
    if !BOOTED.load(Ordering::SeqCst) {
        let _ = boot("");
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);
    let jobs = read(|profile| logic::refresh::due_rss(profile, now));
    let mut done = 0u32;
    for index in jobs {
        if background_expired() {
            eprintln!("[pulse] background refresh expired feeds={done}");
            sync_badge();
            return false;
        }
        let url = read(|profile| logic::model::profile_url_at(profile, "rss", index));
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            continue;
        }
        match fetch_feed(&url) {
            Ok(xml) => {
                let _ = ingest_rss(index, &xml);
                done += 1;
            }
            Err(error) => {
                let error = error.replace('\n', " ");
                eprintln!("[pulse] background rss failed {error}");
            }
        }
    }
    if done > 0 {
        save_now();
    }
    sync_badge();
    eprintln!("[pulse] background refresh feeds={done}");
    !background_expired()
}

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn fetch_feed(url: &str) -> Result<String, String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(6)))
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent
        .get(url)
        .header("User-Agent", "Pulse/0.1 (RSS)")
        .call()
        .map_err(|error| error.to_string())?;
    let mut body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    const CAP: usize = 1_000_000;
    if body.len() > CAP {
        let mut end = CAP;
        while end > 0 && !body.is_char_boundary(end) {
            end -= 1;
        }
        body.truncate(end);
    }
    Ok(body)
}

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn save_now() {
    let json = {
        let guard = lock();
        serde_json::to_string(&*guard.profile).unwrap_or_else(|_| "{}".into())
    };
    let Some(path) = profile_path() else {
        return;
    };
    if let Err(error) = fs::write(&path, json) {
        eprintln!("[pulse] background save failed {error}");
    }
}

pub fn export_profile() -> String {
    read(|profile| serde_json::to_string(profile).unwrap_or_else(|_| "{}".into()))
}

/// Serializes the dashboard off the UI thread. A newer edit starts another
/// save when this one finishes, so the file catches up without a second copy
/// while this one is writing.
pub fn queue_save() {
    let (snapshot, captured) = {
        let mut guard = lock();
        if guard.saving {
            return;
        }
        guard.saving = true;
        guard.saving_revision = guard.revision;
        (Arc::clone(&guard.profile), guard.revision)
    };
    std::thread::spawn(move || {
        let started = Instant::now();
        let json = serde_json::to_string(&*snapshot).unwrap_or_else(|_| "{}".into());
        // Drop the snapshot before the disk write so a click can edit without
        // copying every article while the file is still flushing.
        drop(snapshot);
        if let Some(path) = profile_path() {
            if let Err(error) = fs::write(&path, &json) {
                eprintln!("[pulse] save failed {error}");
            }
        }
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        if ms >= 50.0 {
            eprintln!("[pulse] slow source=save-file ms={ms:.0}");
        }
        let mut guard = lock();
        if guard.revision == captured {
            guard.export_json = json;
            guard.export_revision = captured;
        }
        let again = guard.revision != captured;
        guard.saving = false;
        drop(guard);
        if again {
            queue_save();
        }
    });
}

/// The profile JSON the save thread already built. Empty when that string is
/// not ready, so the UI thread never serializes the library itself.
pub fn take_export() -> String {
    let mut guard = lock();
    if guard.export_json.is_empty() || guard.export_revision != guard.revision {
        return String::new();
    }
    guard.export_revision = 0;
    std::mem::take(&mut guard.export_json)
}

fn dash(applied: bool) -> String {
    let guard = lock();
    dash_of(&guard.profile, true, String::new(), applied, guard.revision)
}

fn dash_err(error: &str) -> String {
    let guard = lock();
    dash_of(&guard.profile, false, error.to_string(), false, guard.revision)
}

fn dash_of(profile: &Profile, ok: bool, error: String, applied: bool, revision: u64) -> String {
    let keywords = profile.settings.mute_keywords.clone();
    let row = |card: &logic::model::Card| DashCard {
        handle: card.handle.clone(),
        label: card.label.clone(),
        last_fetch_ms: card.last_fetch_ms,
        last_attempt_ms: card.last_attempt_ms,
        note: card.note.clone(),
        folder: card.folder.clone(),
        unread: logic::model::card_badge(card, &keywords),
    };
    to_json(&Dash {
        ok,
        error,
        applied,
        revision,
        version: profile.version,
        focus: profile.focus.clone(),
        settings: profile.settings.clone(),
        sync: profile.sync.clone(),
        x: profile.x.cards.iter().map(row).collect(),
        threads: profile.threads.cards.iter().map(row).collect(),
        instagram: profile.instagram.cards.iter().map(row).collect(),
        weibo: profile.weibo.cards.iter().map(row).collect(),
        rss: profile.rss.cards.iter().map(row).collect(),
        folders: profile.folders.clone(),
    })
}

fn platform_rejected(platform: &str) -> bool {
    !matches!(
        platform.trim().to_ascii_lowercase().as_str(),
        "x" | "threads" | "thread" | "instagram" | "ig" | "weibo" | "wb" | "rss" | "feed"
    )
}

fn fingerprint(card: &logic::model::Card) -> u64 {
    let mut hash = card.last_fetch_ms
        ^ ((card.posts.len() as u64) << 24)
        ^ ((card.seen.len() as u64) << 8);
    hash ^= fnv(&card.handle) ^ fnv(&card.label) ^ fnv(&card.note) ^ fnv(&card.avatar);
    for (index, post) in card.posts.iter().enumerate() {
        hash = hash
            .wrapping_mul(16777619)
            .wrapping_add(fnv(&post.id))
            .wrapping_add(index as u64);
    }
    for id in &card.seen {
        hash = hash.wrapping_mul(31).wrapping_add(fnv(id));
    }
    hash
}

fn fnv(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn profile_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut dir = PathBuf::from(home);
    dir.push("Library");
    dir.push("Application Support");
    dir.push(crate::legacy::SUPPORT_DIR);
    fs::create_dir_all(&dir).ok()?;
    dir.push("profile.json");
    Some(dir)
}

fn read_saved() -> Option<String> {
    let path = profile_path()?;
    fs::read_to_string(path).ok()
}

fn empty_present(error: impl Into<String>) -> logic::model::PresentCard {
    logic::model::PresentCard {
        ok: false,
        error: error.into(),
        platform: "x".into(),
        handle: String::new(),
        label: String::new(),
        avatar: String::new(),
        url: String::new(),
        unread: 0,
        note: String::new(),
        last_fetch_ms: 0,
        posts: Vec::new(),
    }
}

fn to_json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".into())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DashCard {
    handle: String,
    label: String,
    last_fetch_ms: u64,
    last_attempt_ms: u64,
    note: String,
    folder: String,
    unread: u32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Dash {
    ok: bool,
    error: String,
    applied: bool,
    revision: u64,
    version: u32,
    focus: String,
    settings: logic::model::Settings,
    x: Vec<DashCard>,
    threads: Vec<DashCard>,
    instagram: Vec<DashCard>,
    weibo: Vec<DashCard>,
    rss: Vec<DashCard>,
    folders: Vec<logic::model::RssFolder>,
    sync: logic::model::SyncPrefs,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FetchReply {
    ok: bool,
    error: String,
    jobs: Vec<logic::schedule::FetchJob>,
    max: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::model::{Card, Post, Profile};
    use std::time::Instant;

    fn fat(cards: usize, posts: usize, url_len: usize) -> Profile {
        let url = format!("https://scontent.cdninstagram.com/{}", "a".repeat(url_len));
        let mut profile = Profile::default();
        profile.focus = "x".into();
        for index in 0..cards {
            let mut card = Card {
                handle: format!("user{index}"),
                ..Card::default()
            };
            for post in 0..posts {
                std::sync::Arc::make_mut(&mut card.posts).push(Post {
                    id: format!("{index}-{post}"),
                    text: "hello".into(),
                    time_ms: post as i64,
                    url: format!("https://x.com/user{index}/status/{post}"),
                    images: vec![url.clone(), url.clone()],
                    ..Post::default()
                });
            }
            profile.x.cards.push(card);
        }
        profile
    }

    #[test]
    fn take_export_is_empty_until_a_save_finishes() {
        assert!(take_export().is_empty());
    }

    #[test]
    fn view_slots_and_finish_stay_under_a_frame() {
        let profile = fat(8, 40, 2500);
        let json = serde_json::to_string(&profile).unwrap();
        let old_slots = Instant::now();
        let parsed: Profile = serde_json::from_str(&json).unwrap();
        let slots = logic::model::focus_slots(&parsed);
        let _slots_json = serde_json::to_string(&slots).unwrap();
        let old_slots_ms = old_slots.elapsed().as_secs_f64() * 1000.0;

        install(profile);
        let new_slots = Instant::now();
        let _live = view_slots();
        let new_slots_ms = new_slots.elapsed().as_secs_f64() * 1000.0;

        let old_finish = Instant::now();
        let mut again: Profile = serde_json::from_str(&json).unwrap();
        logic::model::finish_fetch(&mut again, "x", 0);
        let _back = serde_json::to_string(&again).unwrap();
        let old_finish_ms = old_finish.elapsed().as_secs_f64() * 1000.0;

        let new_finish = Instant::now();
        let _ack = finish_fetch("x", 0);
        let new_finish_ms = new_finish.elapsed().as_secs_f64() * 1000.0;

        eprintln!(
            "[pulse] probe view-slots before={old_slots_ms:.1} after={new_slots_ms:.1} bytes={}",
            json.len()
        );
        eprintln!(
            "[pulse] probe scraper-finish before={old_finish_ms:.1} after={new_finish_ms:.1}"
        );
        assert!(
            new_slots_ms < 16.0,
            "view-slots {new_slots_ms:.1} ms, was {old_slots_ms:.1}"
        );
        assert!(
            new_finish_ms < 16.0,
            "finish {new_finish_ms:.1} ms, was {old_finish_ms:.1}"
        );
    }
}
