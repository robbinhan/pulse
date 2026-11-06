//! JSON door between the screen and the feed logic.

use nsh_core::prelude::*;

use crate::logic::{self, model::Profile};

/// Feed math, the saved dashboard, and the X page script.
#[module]
pub struct Core;

static IMAGE_REJECTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[module]
impl Core {
    /// Parses a card header. JSON `{ kind, supported, handle, url, message }`.
    pub fn parse_source(input: String) -> String {
        let hit = logic::source::parse_source(&input);
        to_json(&SourceReply {
            kind: hit.kind,
            supported: hit.supported,
            handle: hit.handle,
            url: hit.url,
            message: hit.message,
        })
    }

    /// A new 2×2 dashboard with empty cards.
    pub fn default_profile() -> String {
        to_json(&Profile::default())
    }

    /// Copies TheySay's file, storage keys, and website data before the new id is used.
    pub fn prepare_legacy() {
        crate::store::prepare_legacy();
    }

    /// A saved login mark from the previous bundle, when this one's storage has none.
    pub fn legacy_session() -> String {
        crate::store::legacy_session()
    }

    /// Loads a saved dashboard into memory. The JSON is the slim dash, not every post.
    pub fn boot(storage: String) -> String {
        crate::store::boot(&storage)
    }

    /// Checks a saved dashboard and keeps it in memory. JSON is the slim dash.
    pub fn normalize_profile(json: String) -> String {
        match crate::store::adopt_json(&json) {
            Ok(dash) => dash,
            Err(error) => dash_error(&error),
        }
    }

    /// Shows `x`, `threads`, `instagram`, or the mixed `all` view.
    pub fn set_focus(focus: String) -> String {
        crate::store::set_focus(&focus)
    }

    /// `timeline` or `board`. The stream is the home; the board is the account grid.
    pub fn set_home(home: String) -> String {
        crate::store::set_home(&home)
    }

    /// One page of the mixed stream for `focus` (`all` or a platform). Newest first.
    /// `folder` filters the RSS focus: empty is every feed, `-` is 未分类, otherwise a folder id.
    pub fn timeline(focus: String, offset: u32, limit: u32, folder: String) -> String {
        crate::store::timeline(&focus, offset, limit, &folder)
    }

    /// Marks one post read. A repeat is a no-op and does not rebuild the screen.
    pub fn mark_post(platform: String, index: u32, id: String) -> String {
        crate::store::mark_post(&platform, index as usize, &id)
    }

    /// Sets one card's @handle on that platform only. An empty string removes it.
    /// An index equal to the list length appends.
    pub fn set_handle(platform: String, index: u32, input: String) -> String {
        crate::store::set_handle(&platform, index as usize, &input)
    }

    /// The login page for `x`, `threads`, or `instagram`.
    pub fn login_url(platform: String) -> String {
        logic::source::login_url(&platform)
    }

    /// Login page for `os` (`ios` keeps the mobile Weibo login; other desktops get the QR passport).
    pub fn login_page(platform: String, os: String) -> String {
        logic::source::login_page(&platform, &os)
    }

    /// `[pulse] click action=commit platform=… value=…`
    pub fn note_commit(platform: String, value: String) {
        let value: String = value
            .chars()
            .filter(|char| *char != '\n' && *char != '\r')
            .take(180)
            .collect();
        eprintln!("[pulse] click action=commit platform={platform} value={value}");
    }

    /// `{ ok, needsResolve, uid, q, via, error }` for the Weibo add field.
    pub fn weibo_query(input: String) -> String {
        match logic::source::read_weibo_input(&input) {
            logic::source::WeiboInput::Uid { uid, query } => serde_json::json!({
                "ok": true,
                "needsResolve": false,
                "uid": uid,
                "q": query,
                "via": if query.contains('/') { "url" } else { "uid" },
                "name": "",
                "avatar": "",
                "error": ""
            })
            .to_string(),
            logic::source::WeiboInput::Name { query } => serde_json::json!({
                "ok": true,
                "needsResolve": true,
                "uid": "",
                "q": query,
                "via": "name",
                "name": "",
                "avatar": "",
                "error": ""
            })
            .to_string(),
            logic::source::WeiboInput::Reject { message } => serde_json::json!({
                "ok": false,
                "needsResolve": false,
                "uid": "",
                "q": "",
                "via": "",
                "name": "",
                "avatar": "",
                "error": message
            })
            .to_string(),
        }
    }

    /// Name and photo from a profile `getIndex` body. Empty strings when absent.
    pub fn weibo_profile(raw: String) -> String {
        let feed = logic::weibo::parse_index(&raw, "");
        serde_json::json!({
            "name": feed.display_name,
            "avatar": feed.avatar
        })
        .to_string()
    }

    /// Exact `screen_name` from a Weibo user-search payload.
    pub fn weibo_match(query: String, raw: String) -> String {
        match logic::weibo::find_user(&raw, &query) {
            Some(user) => serde_json::json!({
                "ok": true,
                "uid": user.uid,
                "name": user.screen_name,
                "avatar": user.avatar,
                "error": ""
            })
            .to_string(),
            None => serde_json::json!({
                "ok": false,
                "uid": "",
                "name": "",
                "avatar": "",
                "error": "没找到这个微博账号"
            })
            .to_string(),
        }
    }

    /// Uid in `/u/<uid>` or `/profile/<uid>`, else empty.
    pub fn weibo_uid_in_url(url: String) -> String {
        logic::source::weibo_uid_in_url(&url)
    }

    /// Parses one fetched feed into its card.
    pub fn ingest_rss(index: u32, xml: String) -> String {
        crate::store::ingest_rss(index as usize, &xml)
    }

    /// Adds every new `xmlUrl` in an OPML document.
    pub fn import_opml(xml: String) -> String {
        crate::store::import_opml(&xml)
    }

    /// The RSS list as OPML.
    pub fn export_opml() -> String {
        crate::store::export_opml()
    }

    /// Subscription list only: RSS feeds, folders, and followed accounts. No posts or read marks.
    pub fn export_subscriptions() -> String {
        crate::store::export_subscriptions()
    }

    /// Unions a subscription list into this device. Existing posts and read marks stay.
    pub fn merge_subscriptions(json: String) -> String {
        crate::store::merge_subscriptions(&json)
    }

    /// A local secret the user copies onto their own sync endpoint. Nothing is uploaded here.
    pub fn new_sync_token() -> String {
        crate::store::new_sync_token()
    }

    /// `mode` is `off`, `icloud`, or `https`. URL and token are kept for the https mode.
    pub fn set_sync(mode: String, url: String, token: String, pull_on_open: bool) -> String {
        crate::store::set_sync(&mode, &url, &token, pull_on_open)
    }

    /// Whether this signed build has an iCloud container. Sideload packages do not.
    pub fn icloud_status() -> String {
        crate::store::icloud_status()
    }

    /// Merges the subscription list stored in the iCloud container.
    pub fn icloud_pull() -> String {
        crate::store::icloud_pull()
    }

    /// Writes the subscription list into the iCloud container.
    pub fn icloud_push() -> String {
        crate::store::icloud_push()
    }

    /// Records that a subscription sync just succeeded.
    pub fn note_sync() -> String {
        crate::store::note_sync()
    }

    /// Opens the system file panel, reads one `.opml` or `.xml`, and imports it.
    /// Cancel and a missing panel return a small JSON object, not a dashboard.
    pub fn import_opml_file() -> String {
        crate::store::import_opml_file()
    }

    /// Writes the current RSS list through the system save panel.
    pub fn export_opml_file() -> String {
        crate::store::export_opml_file()
    }

    /// RSS cards without article bodies: title, host, post count, empty flag.
    pub fn rss_catalog() -> String {
        crate::store::rss_catalog()
    }

    /// Removes the feeds whose URLs are in a JSON string array.
    pub fn remove_rss(urls_json: String) -> String {
        crate::store::remove_rss(&urls_json)
    }

    /// Removes every RSS card that has no stored posts.
    pub fn clear_empty_rss() -> String {
        crate::store::clear_empty_rss()
    }

    /// Creates an RSS folder and returns the dashboard.
    pub fn create_rss_folder(name: String) -> String {
        crate::store::create_rss_folder(&name)
    }

    /// Renames an RSS folder. Feeds keep the same id.
    pub fn rename_rss_folder(id: String, name: String) -> String {
        crate::store::rename_rss_folder(&id, &name)
    }

    /// Deletes an RSS folder. Its feeds become 未分类.
    pub fn delete_rss_folder(id: String) -> String {
        crate::store::delete_rss_folder(&id)
    }

    /// Moves the feeds in a JSON URL array into `folder_id` (`""` is 未分类).
    pub fn move_rss(urls_json: String, folder_id: String) -> String {
        crate::store::move_rss(&urls_json, &folder_id)
    }

    /// Puts one account or feed into a shared folder. Empty id is 未分类.
    pub fn move_card_folder(platform: String, handle: String, folder_id: String) -> String {
        crate::store::move_card_folder(&platform, &handle, &folder_id)
    }

    /// Saves a Weibo uid onto that list and logs the resolve.
    pub fn add_weibo(index: u32, uid: String, label: String, avatar: String, query: String, via: String) -> String {
        crate::store::add_weibo(index as usize, &uid, &label, &avatar, &query, &via)
    }

    /// A Weibo login-sheet navigation or a resource that failed to load.
    pub fn weibo_login_log(line: String) {
        let line: String = line.chars().filter(|c| *c != '\n' && *c != '\r').take(400).collect();
        eprintln!("[pulse] weibo-login {line}");
    }

    /// The home page used to check that platform's session.
    pub fn home_url(platform: String) -> String {
        logic::source::home_url(&platform)
    }

    /// Sets the refresh interval in seconds.
    pub fn set_refresh(seconds: u32) -> String {
        crate::store::set_refresh(seconds)
    }

    /// Replaces mute keywords. Separate words with commas or new lines.
    pub fn set_mute(keywords: String) -> String {
        crate::store::set_mute(&keywords)
    }

    /// Turns system notifications on or off for every platform.
    pub fn set_notify(on: bool) -> String {
        crate::store::set_notify(on)
    }

    /// Turns notifications on or off for `x`, `threads`, or `instagram`.
    pub fn set_notify_platform(platform: String, on: bool) -> String {
        crate::store::set_notify_platform(&platform, on)
    }

    /// Dock badge from the unread total, and notification permission on first use.
    pub fn sync_chrome() {
        crate::notify::ensure_auth();
        crate::store::sync_badge();
        crate::notify::maybe_test_from_env();
    }

    /// Writes the dashboard on a background thread.
    pub fn queue_save() {
        crate::store::queue_save();
    }

    /// The full dashboard, for the copy button.
    pub fn export_profile() -> String {
        crate::store::export_profile()
    }

    /// Profile JSON already built on the save thread. Empty until that save matches
    /// the current revision. Does not serialize.
    pub fn take_export() -> String {
        crate::store::take_export()
    }

    /// A notification click, `{ platform, handle }`, or an empty string.
    pub fn poll_notice() -> String {
        match crate::notify::poll_open() {
            Some((platform, handle)) => serde_json::json!({ "platform": platform, "handle": handle }).to_string(),
            None => String::new(),
        }
    }

    /// Posts one sample notification so the Mac path can be checked.
    pub fn test_notify() {
        crate::notify::ensure_auth();
        crate::notify::deliver_test();
    }

    /// `light`, `dark`, or `system`.
    pub fn set_theme(theme: String) -> String {
        crate::store::set_theme(&theme)
    }

    /// Target language, API base, bearer key, and chat model. The key stays on this device.
    pub fn set_translate(lang: String, url: String, token: String, model: String) -> String {
        crate::store::set_translate(&lang, &url, &token, &model)
    }

    /// Stars or unstars one post. The copy lives in the local profile.
    pub fn toggle_star(platform: String, id: String) -> String {
        crate::store::toggle_star(&platform, &id)
    }

    /// Timeline of starred posts, newest star first.
    pub fn starred_timeline(offset: u32, limit: u32) -> String {
        crate::store::starred_timeline(offset as usize, limit as usize)
    }

    /// Local hour for the morning briefing, 0 through 23.
    pub fn set_briefing_hour(hour: u32) -> String {
        crate::store::set_briefing_hour(hour)
    }

    /// Whether this device generates the morning briefing on its own.
    pub fn set_briefing_auto(on: bool) -> String {
        crate::store::set_briefing_auto(on)
    }

    /// The saved morning script, if one was written.
    pub fn briefing() -> String {
        crate::store::briefing_status()
    }

    /// Writes today's briefing from stars in `[from_ms, until_ms)`. `day` is the local `YYYY-MM-DD`.
    /// A repeat the same day does nothing unless `force` is set. Uses the translation API settings.
    pub async fn make_briefing(from_ms: f64, until_ms: f64, day: String, force: bool) -> String {
        let from_ms = from_ms as i64;
        let until_ms = until_ms as i64;
        let work = blocking(move || crate::store::make_briefing(from_ms, until_ms, &day, force));
        match work.await {
            Ok(text) => text,
            Err(_) => r#"{"ok":false,"empty":true,"script":"","note":"早报没有写成。","count":0}"#.into(),
        }
    }

    /// Speaks the script with the system Chinese voice.
    pub fn speak(text: String) -> String {
        match crate::speech::speak(&text) {
            Ok(()) => r#"{"ok":true,"error":""}"#.into(),
            Err(error) => serde_json::json!({ "ok": false, "error": error }).to_string(),
        }
    }

    /// Stops the system voice.
    pub fn stop_speak() {
        crate::speech::stop();
    }

    /// Translates one post off the UI thread. JSON `{ ok, text, error, cached }`.
    /// `locale` is the system BCP 47 tag, used only when the setting is `system`.
    pub async fn translate(post_id: String, title: String, body: String, quote: String, locale: String) -> String {
        let work = blocking(move || crate::translate::translate_post(&post_id, &title, &body, &quote, &locale));
        match work.await {
            Ok(text) => text,
            Err(_) => {
                logic::translate::Outcome {
                    ok: false,
                    text: String::new(),
                    error: "翻译没有完成。".into(),
                    cached: false,
                }
                .json()
            }
        }
    }

    /// Folds one web-view message into the card kept in memory.
    pub fn ingest(platform: String, index: u32, message: String) -> String {
        crate::store::ingest(&platform, index as usize, &message)
    }

    /// Marks one card's current posts as read.
    pub fn mark_read(platform: String, index: u32) -> String {
        crate::store::mark_read(&platform, index as usize)
    }

    /// Marks every visible card as read.
    pub fn mark_all_read() -> String {
        crate::store::mark_all_read()
    }

    /// Marks visible cards due so the next pass reads them again.
    pub fn request_refresh() -> String {
        crate::store::request_refresh()
    }

    /// Records that a card finished a fetch at `nowMs`.
    pub fn touch_fetch(json: String, platform: String, index: u32, now_ms: f64) -> String {
        let Some(now_ms) = finite_ms(now_ms) else {
            return err("时间无效。");
        };
        edit(json, |profile| {
            logic::model::touch_fetch(profile, &platform, index as usize, now_ms)
        })
    }

    /// Moves one card up (`direction` negative) or down within its platform.
    pub fn move_card(platform: String, index: u32, direction: i32) -> String {
        crate::store::move_card(&platform, index as usize, direction)
    }

    /// Cards that should open a hidden web view now.
    /// `busyJson` is a list of `platform:index` keys.
    pub fn next_fetches(now_ms: f64, busy_json: String, from: u32, until: u32) -> String {
        let Some(now_ms) = finite_ms(now_ms) else {
            return to_json(&FetchReply {
                ok: false,
                error: "时间无效。".into(),
                jobs: Vec::new(),
                max: 0,
            });
        };
        let busy = match serde_json::from_str::<Vec<String>>(&busy_json) {
            Ok(busy) => busy,
            Err(_) => {
                return to_json(&FetchReply {
                    ok: false,
                    error: "忙碌列表无法读取。".into(),
                    jobs: Vec::new(),
                    max: 0,
                });
            }
        };
        crate::store::next_fetches(now_ms, &busy, from, until)
    }

    /// `before` and `after` are the card post-list offsets around a refresh.
    pub fn note_scroll(card: String, before: f64, after: f64) {
        eprintln!("[pulse] scroll card={card} before={before:.0} after={after:.0}");
    }

    /// Milliseconds from a header click until the screen state changed.
    pub fn note_click(action: String, ms: f64) {
        let ms = if ms.is_finite() { ms.max(0.0) } else { 0.0 };
        eprintln!("[pulse] click action={action} ms={ms:.1}");
    }

    /// A stretch of JS work that blocked the UI thread.
    pub fn note_slow(source: String, ms: f64) {
        let ms = if ms.is_finite() { ms.max(0.0) } else { 0.0 };
        eprintln!("[pulse] slow source={source} ms={ms:.0}");
    }

    /// Where the card's post list should scroll after it grows.
    /// `-1` means leave the offset alone. The bottom is followed only when
    /// the reader was already within 24px of it.
    pub fn scroll_offset(
        prev_offset: f64,
        offset: f64,
        viewport: f64,
        old_content: f64,
        new_content: f64,
    ) -> f64 {
        logic::scroll::next_offset(prev_offset, offset, viewport, old_content, new_content)
    }

    /// Closes one profile fetch: one log line, and the card's retry clock.
    pub fn finish_fetch(platform: String, index: u32) -> String {
        crate::store::finish_fetch(&platform, index as usize)
    }

    /// One card, ready to draw: bubbles, badges, unread count.
    pub fn present_card(platform: String, index: u32) -> String {
        crate::store::present_card(&platform, index as usize)
    }

    /// Handle and post ids, without image URLs, for the scroll clock.
    pub fn card_clock(platform: String, index: u32) -> String {
        crate::store::card_clock(&platform, index as usize)
    }

    /// The profile page a hidden web view should open.
    pub fn profile_url(platform: String, index: u32) -> String {
        crate::store::profile_url(&platform, index as usize)
    }

    /// The accounts in the focused list.
    pub fn view_slots() -> String {
        crate::store::view_slots()
    }

    /// Unread posts on each platform.
    pub fn unread_totals() -> String {
        crate::store::unread_totals()
    }

    /// Unread posts on the focused list.
    pub fn focused_unread() -> String {
        crate::store::focused_unread()
    }

    /// Newest fetch time on the focused list, in milliseconds.
    pub fn focused_fresh_ms() -> String {
        crate::store::focused_fresh_ms()
    }

    /// Synthetic bubbles for an empty card. Not a real timeline.
    pub fn sample_feed() -> String {
        to_json(&logic::model::sample_posts())
    }

    /// `grid` or `column`, from the platform and the window width.
    pub fn shell_mode(os: String, idiom: String, width: f64) -> String {
        logic::schedule::shell_mode(&os, &idiom, width).to_string()
    }

    /// JavaScript for a hidden profile web view. `token` is echoed back.
    pub fn extract_script(token: String) -> String {
        logic::extract::extract_script(&logic::extract::safe_token(&token))
    }

    /// X, Threads, and Discord. Only a supported source loads a page.
    pub fn planned_sources() -> String {
        let rows: Vec<PlanReply> = logic::source::planned_sources()
            .into_iter()
            .map(|plan| PlanReply {
                id: plan.id,
                supported: plan.supported,
                example: plan.example,
            })
            .collect();
        to_json(&rows)
    }

    /// How many profile web views may be open at once.
    pub fn max_scrapers() -> u32 {
        logic::model::MAX_SCRAPERS as u32
    }

    /// How long one profile page stays open, in milliseconds.
    pub fn scraper_dwell_ms() -> u32 {
        logic::model::SCRAPER_DWELL_MS
    }

    /// `in` when `url` is that platform's home and `has_marker` is true, `pending` while home has no marker yet. A login URL is `out`. Any other page is `pending`.
    pub fn session_from_probe(platform: String, url: String, has_marker: bool) -> String {
        logic::session::classify_session(&platform, &url, has_marker).to_string()
    }

    /// `in` for a marker or a viewer, `out` only for a login URL or a login wall, else `pending`.
    pub fn session_from_signals(
        platform: String,
        url: String,
        has_marker: bool,
        login_wall: bool,
        viewer: bool,
    ) -> String {
        logic::session::session_update(logic::session::probe_verdict(
            &platform,
            &url,
            has_marker,
            login_wall,
            viewer,
        ))
        .unwrap_or("")
        .to_string()
    }

    /// `in`, `out`, or `blocked` when `message` is a real login signal, else empty.
    /// Public posts do not count.
    pub fn session_from_scrape(message: String) -> String {
        logic::session::session_from_scrape(&message)
            .unwrap_or("")
            .to_string()
    }

    /// `in` or `out` from a Weibo `api/config` body, else empty.
    pub fn weibo_config_state(raw: String) -> String {
        logic::session::weibo_config_state(&raw)
            .unwrap_or("")
            .to_string()
    }

    /// A post photo as a `data:` URI, fetched with a browser user agent and the right Referer.
    /// Empty when the URL is not a known CDN or the download fails. Cached in memory.
    pub async fn load_media(url: String, platform: String) -> String {
        if let Some(hit) = logic::media::cached(&url) {
            return hit;
        }
        let allowed = logic::media::accept_url(&url)
            || (platform == "rss" && logic::media::image_extension(&url));
        if !allowed {
            let count = IMAGE_REJECTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if count < 3 {
                eprintln!("[pulse] image reject url_len={}", url.len());
            }
            return String::new();
        }
        let mut last_status = 0u16;
        let mut last_type = String::new();
        for referer in logic::media::referers(&platform, &url) {
            let request = HttpRequest::get(&url)
                .header("User-Agent", logic::media::USER_AGENT)
                .header("Referer", referer)
                .header("Accept", "image/avif,image/webp,image/*,*/*;q=0.8");
            let Ok(response) = fetch(request).await else {
                eprintln!("[pulse] image fetch failed referer={referer} url_len={}", url.len());
                continue;
            };
            last_status = response.status();
            last_type = response.header("content-type").unwrap_or("").to_string();
            eprintln!(
                "[pulse] image fetch status={last_status} type={last_type} bytes={} url_len={}",
                response.body.len(),
                url.len()
            );
            if !response.ok() {
                continue;
            }
            let bytes = response.body;
            if bytes.len() < 32 || bytes.len() > logic::media::MAX_BYTES {
                continue;
            }
            let Some(mime) = logic::media::sniff_mime(&last_type, &bytes) else {
                continue;
            };
            let uri = logic::media::data_uri(mime, &bytes);
            logic::media::remember(url.clone(), uri.clone());
            return uri;
        }
        eprintln!(
            "[pulse] image failed status={last_status} type={last_type} url_len={}",
            url.len()
        );
        String::new()
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceReply {
    kind: &'static str,
    supported: bool,
    handle: String,
    url: String,
    message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    ok: bool,
    error: String,
    applied: bool,
    profile: Profile,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanReply {
    id: &'static str,
    supported: bool,
    example: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FetchReply {
    ok: bool,
    error: String,
    jobs: Vec<logic::schedule::FetchJob>,
    max: u32,
}

fn dash_error(error: &str) -> String {
    serde_json::json!({
        "ok": false,
        "error": error,
        "applied": false,
        "revision": 0
    })
    .to_string()
}

fn load(json: &str) -> Result<Profile, String> {
    logic::model::normalize_json(json)
}

fn edit(json: String, change: impl FnOnce(Profile) -> Result<Profile, String>) -> String {
    let profile = match load(&json) {
        Ok(profile) => profile,
        Err(error) => return err(error),
    };
    match change(profile) {
        Ok(profile) => ok(profile, true),
        Err(error) => err(error),
    }
}

fn ok(profile: Profile, applied: bool) -> String {
    to_json(&Envelope {
        ok: true,
        error: String::new(),
        applied,
        profile,
    })
}

fn err(error: impl Into<String>) -> String {
    to_json(&Envelope {
        ok: false,
        error: error.into(),
        applied: false,
        profile: Profile::default(),
    })
}

fn finite_ms(now_ms: f64) -> Option<i64> {
    now_ms.is_finite().then_some(now_ms.max(0.0) as i64)
}

fn to_json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".into())
}
