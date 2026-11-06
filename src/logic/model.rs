//! The dashboard document: one list of accounts per platform, a small cache, and settings.
//!
//! X, Threads, and Instagram each keep their own accounts, in order, with no
//! empty slots. An older save is split by the platform already stored on each
//! card, then empty slots and any grid size are dropped.
//! Stored posts are public text the page already showed. The document never
//! holds cookies or passwords.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::source;
use super::text::{self, Segment};
use super::time::parse_time_ms;

pub const VERSION: u32 = 1;
/// A safety cap so a list cannot fill the 5 MB store by itself.
pub const MAX_CARDS: usize = 200;
pub const MAX_FOLDERS: usize = 40;
pub const MAX_POSTS: usize = 40;
/// A feed library of about 200 subscriptions stays movable if each card keeps a short tail.
pub const MAX_RSS_POSTS: usize = 24;
/// Stored article text. The card shows this much; the link still opens the full page.
pub const RSS_TEXT_CHARS: usize = 2000;
pub const MAX_SEEN: usize = 200;
pub const MAX_STARS: usize = 200;
pub const MAX_CACHE: usize = 24;
pub const MAX_SCRAPERS: usize = 2;
pub const MIN_REFRESH_SECS: u64 = 30;
pub const MAX_REFRESH_SECS: u64 = 3600;
pub const DEFAULT_REFRESH_SECS: u64 = 120;
pub const PREVIEW_CHARS: usize = 220;
pub const SCRAPER_DWELL_MS: u32 = 7500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    pub version: u32,
    /// `x`, `threads`, `instagram`, or `all`.
    pub focus: String,
    pub x: Board,
    pub threads: Board,
    pub instagram: Board,
    /// Older saves omit this. Empty until a Weibo account is added.
    #[serde(default)]
    pub weibo: Board,
    /// RSS and Atom feeds. The card handle is the feed URL. Older saves omit this.
    #[serde(default)]
    pub rss: Board,
    /// Shared directories. Any account or feed can point at one by id.
    /// Older saves kept these on `rss.folders`; they are lifted on open.
    #[serde(default)]
    pub folders: Vec<RssFolder>,
    pub cache: Vec<CachedFeed>,
    pub settings: Settings,
    /// Subscription-list sync. Older saves omit this, which means off.
    #[serde(default)]
    pub sync: SyncPrefs,
    /// Bookmarks. A copy of the post text, so a star outlives the card's short tail.
    /// The subscription sync does not carry posts, and this list stays in the local profile.
    #[serde(default)]
    pub stars: Vec<Star>,
    /// The latest morning script. Empty until one is written.
    #[serde(default)]
    pub briefing: Briefing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Board {
    pub cards: Vec<Card>,
    /// Legacy location. New saves keep folders on the profile. Lifted on open.
    #[serde(default)]
    pub folders: Vec<RssFolder>,
}

/// A shared directory. `id` stays put when the name changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct RssFolder {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub refresh_secs: u64,
    pub mute_keywords: Vec<String>,
    pub theme: String,
    /// Global notification switch. Missing in an older save, which means on.
    #[serde(default = "default_on")]
    pub notify: bool,
    #[serde(default = "default_on")]
    pub notify_x: bool,
    #[serde(default = "default_on")]
    pub notify_threads: bool,
    #[serde(default = "default_on")]
    pub notify_instagram: bool,
    /// Older saves omit this, which means on.
    #[serde(default = "default_on")]
    pub notify_weibo: bool,
    /// Older saves omit this, which means on.
    #[serde(default = "default_on")]
    pub notify_rss: bool,
    /// `timeline` (the mixed stream) or `board` (the account grid). Missing means the stream.
    #[serde(default = "default_home")]
    pub home: String,
    /// `zh`, `en`, `ja`, or `system`. Missing means Simplified Chinese.
    #[serde(default = "default_translate_lang")]
    pub translate_lang: String,
    /// OpenAI-compatible API base, such as `https://api.openai.com/v1`. Empty means no request.
    #[serde(default)]
    pub translate_url: String,
    /// Bearer key for that API. Stays on this device; never part of the subscription list.
    #[serde(default)]
    pub translate_token: String,
    /// Chat model. Missing means `gpt-4o-mini`.
    #[serde(default = "default_translate_model")]
    pub translate_model: String,
    /// Local hour when the morning briefing may generate. Missing means 8.
    #[serde(default = "default_briefing_hour")]
    pub briefing_hour: u8,
    /// This device writes the morning script on its own. Missing means on, so older saves keep generating.
    #[serde(default = "default_on")]
    pub briefing_auto: bool,
}

/// A bookmarked post. `starred_ms` is when the user starred it, not when it was posted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Star {
    pub id: String,
    pub platform: String,
    pub handle: String,
    pub author: String,
    pub title: String,
    pub body: String,
    pub url: String,
    pub starred_ms: i64,
}

impl Default for Star {
    fn default() -> Self {
        Self {
            id: String::new(),
            platform: String::new(),
            handle: String::new(),
            author: String::new(),
            title: String::new(),
            body: String::new(),
            url: String::new(),
            starred_ms: 0,
        }
    }
}

/// Cached morning script. `day` is the local calendar day it was prepared for (`YYYY-MM-DD`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Briefing {
    pub day: String,
    pub script: String,
    pub note: String,
    pub count: u32,
}

impl Default for Briefing {
    fn default() -> Self {
        Self {
            day: String::new(),
            script: String::new(),
            note: String::new(),
            count: 0,
        }
    }
}

/// Where the subscription list is copied. Posts and read marks stay local.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncPrefs {
    /// `off`, `icloud`, or `https`.
    pub mode: String,
    pub url: String,
    pub token: String,
    pub pull_on_open: bool,
    pub last_sync_ms: u64,
}

fn default_home() -> String {
    "timeline".into()
}

fn default_translate_lang() -> String {
    "zh".into()
}

fn default_translate_model() -> String {
    "gpt-4o-mini".into()
}

fn default_briefing_hour() -> u8 {
    8
}

fn default_on() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Card {
    pub handle: String,
    pub label: String,
    /// Profile photo URL. Empty until a scrape sees one.
    pub avatar: String,
    /// Shared across a save snapshot. Cloning the profile bumps this arc
    /// instead of copying every article.
    pub posts: Arc<Vec<Post>>,
    pub seen: Vec<String>,
    pub last_fetch_ms: u64,
    /// When a fetch last started or finished, including an empty one.
    /// The scheduler uses this so a failing card waits out its backoff.
    pub last_attempt_ms: u64,
    /// Empty or failed reads in a row. A real parse clears it.
    pub fail_streak: u32,
    /// Newest post time already considered for notifications. `0` means not seeded.
    pub notify_ms: i64,
    pub note: String,
    /// RSS folder id. Empty means the feed is not in a folder. Other platforms leave this empty.
    #[serde(default)]
    pub folder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CachedFeed {
    pub platform: String,
    pub handle: String,
    pub label: String,
    pub avatar: String,
    pub posts: Vec<Post>,
    pub seen: Vec<String>,
    pub last_fetch_ms: u64,
    pub last_attempt_ms: u64,
    pub fail_streak: u32,
    pub notify_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Post {
    pub id: String,
    pub text: String,
    pub time_ms: i64,
    pub url: String,
    pub images: Vec<String>,
    pub link_title: String,
    pub link_url: String,
    pub quote: Option<Quote>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Quote {
    pub author: String,
    pub text: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JsonSegment {
    pub kind: String,
    pub text: String,
    pub symbol: String,
    pub tone: u8,
}

impl From<Segment> for JsonSegment {
    fn from(segment: Segment) -> Self {
        Self {
            kind: segment.kind.to_string(),
            text: segment.text,
            symbol: segment.symbol,
            tone: segment.tone,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PresentQuote {
    pub author: String,
    pub url: String,
    pub segments: Vec<JsonSegment>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PresentPost {
    pub id: String,
    pub segments: Vec<JsonSegment>,
    pub time_ms: i64,
    pub url: String,
    pub images: Vec<String>,
    pub link_title: String,
    pub link_url: String,
    pub quote: Option<PresentQuote>,
    pub unread: bool,
    pub starred: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PresentCard {
    pub ok: bool,
    pub error: String,
    pub platform: String,
    pub handle: String,
    pub label: String,
    pub avatar: String,
    pub url: String,
    pub unread: u32,
    pub note: String,
    pub last_fetch_ms: u64,
    pub posts: Vec<PresentPost>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            version: VERSION,
            focus: "all".into(),
            x: Board::default(),
            threads: Board::default(),
            instagram: Board::default(),
            weibo: Board::default(),
            rss: Board::default(),
            folders: Vec::new(),
            cache: Vec::new(),
            settings: Settings::default(),
            sync: SyncPrefs::default(),
            stars: Vec::new(),
            briefing: Briefing::default(),
        }
    }
}

impl Default for SyncPrefs {
    fn default() -> Self {
        Self {
            mode: "off".into(),
            url: String::new(),
            token: String::new(),
            pull_on_open: false,
            last_sync_ms: 0,
        }
    }
}

impl Default for Board {
    fn default() -> Self {
        Self {
            cards: Vec::new(),
            folders: Vec::new(),
        }
    }
}

impl Default for RssFolder {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            refresh_secs: DEFAULT_REFRESH_SECS,
            mute_keywords: Vec::new(),
            theme: "system".into(),
            notify: true,
            notify_x: true,
            notify_threads: true,
            notify_instagram: true,
            notify_weibo: true,
            notify_rss: true,
            home: default_home(),
            translate_lang: default_translate_lang(),
            translate_url: String::new(),
            translate_token: String::new(),
            translate_model: default_translate_model(),
            briefing_hour: default_briefing_hour(),
            briefing_auto: true,
        }
    }
}

impl Default for Card {
    fn default() -> Self {
        Self {
            handle: String::new(),
            label: String::new(),
            avatar: String::new(),
            posts: Arc::new(Vec::new()),
            seen: Vec::new(),
            last_fetch_ms: 0,
            last_attempt_ms: 0,
            fail_streak: 0,
            notify_ms: 0,
            note: String::new(),
            folder: String::new(),
        }
    }
}

impl Default for CachedFeed {
    fn default() -> Self {
        Self {
            platform: "x".into(),
            handle: String::new(),
            label: String::new(),
            avatar: String::new(),
            posts: Vec::new(),
            seen: Vec::new(),
            last_fetch_ms: 0,
            last_attempt_ms: 0,
            fail_streak: 0,
            notify_ms: 0,
        }
    }
}

impl Default for Post {
    fn default() -> Self {
        Self {
            id: String::new(),
            text: String::new(),
            time_ms: 0,
            url: String::new(),
            images: Vec::new(),
            link_title: String::new(),
            link_url: String::new(),
            quote: None,
        }
    }
}

impl Default for Quote {
    fn default() -> Self {
        Self {
            author: String::new(),
            text: String::new(),
            url: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Incoming {
    #[serde(rename = "type")]
    kind: String,
    state: String,
    handle: String,
    display_name: String,
    avatar: String,
    detail: String,
    inlines: Vec<InlineImage>,
    scripts: Vec<String>,
    sjs_count: u32,
    html: String,
    posts: Vec<IncomingPost>,
    /// Raw JSON from a site API (`web_profile_info`, Weibo `getIndex`).
    raw: String,
    /// Milliseconds the page spent loading before this message.
    load_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct InlineImage {
    url: String,
    data: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
struct IncomingPost {
    id: String,
    text: String,
    time: String,
    time_ms: Option<i64>,
    url: String,
    images: Vec<String>,
    link_title: String,
    link_url: String,
    quote: Option<Quote>,
}

impl Default for Incoming {
    fn default() -> Self {
        Self {
            kind: String::new(),
            state: String::new(),
            handle: String::new(),
            display_name: String::new(),
            avatar: String::new(),
            detail: String::new(),
            inlines: Vec::new(),
            scripts: Vec::new(),
            sjs_count: 0,
            html: String::new(),
            posts: Vec::new(),
            raw: String::new(),
            load_ms: 0,
        }
    }
}

impl Default for InlineImage {
    fn default() -> Self {
        Self {
            url: String::new(),
            data: String::new(),
        }
    }
}

impl Default for IncomingPost {
    fn default() -> Self {
        Self {
            id: String::new(),
            text: String::new(),
            time: String::new(),
            time_ms: None,
            url: String::new(),
            images: Vec::new(),
            link_title: String::new(),
            link_url: String::new(),
            quote: None,
        }
    }
}

impl IncomingPost {
    fn into_post(self) -> Post {
        let time_ms = self
            .time_ms
            .filter(|ms| *ms > 0)
            .or_else(|| parse_time_ms(&self.time))
            .unwrap_or(0);
        Post {
            id: self.id,
            text: self.text,
            time_ms,
            url: self.url,
            images: self.images,
            link_title: self.link_title,
            link_url: self.link_url,
            quote: self.quote,
        }
    }
}

pub fn normalize_json(json: &str) -> Result<Profile, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| "JSON 无法读取。".to_string())?;
    if !value.is_object() {
        return Err("配置需要是一个 JSON 对象。".into());
    }
    migrate_legacy_cards(&mut value);
    migrate_timeline_home(&mut value);
    let profile: Profile =
        serde_json::from_value(value).map_err(|_| "配置里有读不懂的字段。".to_string())?;
    if profile.version != VERSION {
        return Err("不是这个版本能读的配置。".into());
    }
    Ok(scrub_profile(profile))
}

/// Older saves keep one shared `cards` array. Each card already names a
/// platform, so those accounts move onto that platform's own list, in the
/// order they appear. Empty slots are dropped. A grid size on the file is ignored.
fn migrate_legacy_cards(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if obj.get("x").and_then(|board| board.get("cards")).is_some() {
        obj.remove("layout");
        obj.remove("allLayout");
        return;
    }
    let old_cards = obj.remove("cards").unwrap_or(serde_json::Value::Array(Vec::new()));
    obj.remove("layout");
    obj.remove("allLayout");
    let mut grouped: [Vec<serde_json::Value>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    if let Some(cards) = old_cards.as_array() {
        for card in cards {
            let handle = card
                .get("handle")
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .trim();
            if handle.is_empty() {
                continue;
            }
            let platform = card
                .get("platform")
                .and_then(|item| item.as_str())
                .unwrap_or("x");
            let bucket = match source::canon_platform(platform) {
                "threads" => 1,
                "instagram" => 2,
                "weibo" => 3,
                _ => 0,
            };
            if grouped[bucket].len() < MAX_CARDS {
                grouped[bucket].push(card.clone());
            }
        }
    }
    if !obj.contains_key("focus") {
        obj.insert("focus".into(), serde_json::Value::String("x".into()));
    }
    for (key, cards) in [("x", 0), ("threads", 1), ("instagram", 2), ("weibo", 3)] {
        obj.insert(
            key.into(),
            serde_json::json!({ "cards": grouped[cards] }),
        );
    }
}

pub fn scrub_profile(mut profile: Profile) -> Profile {
    profile.version = VERSION;
    profile.focus = canon_focus(&profile.focus).to_string();
    profile.sync.mode = canon_sync_mode(&profile.sync.mode).to_string();
    profile.sync.url = profile.sync.url.trim().to_string();
    profile.sync.token = profile.sync.token.trim().to_string();
    profile.settings.refresh_secs = profile
        .settings
        .refresh_secs
        .clamp(MIN_REFRESH_SECS, MAX_REFRESH_SECS);
    profile.settings.theme = canon_theme(&profile.settings.theme).to_string();
    profile.settings.mute_keywords = parse_keywords(&profile.settings.mute_keywords.join("\n"));
    let keywords = profile.settings.mute_keywords.clone();
    scrub_board(&mut profile.x, "x", &keywords);
    scrub_board(&mut profile.threads, "threads", &keywords);
    scrub_board(&mut profile.instagram, "instagram", &keywords);
    scrub_board(&mut profile.weibo, "weibo", &keywords);
    scrub_board(&mut profile.rss, "rss", &keywords);
    lift_folders(&mut profile);
    scrub_stars(&mut profile);
    profile.settings.briefing_hour = profile.settings.briefing_hour.min(23);
    profile
        .cache
        .retain(|item| source::valid_handle(&item.platform, &item.handle));
    for item in &mut profile.cache {
        item.platform = source::canon_platform(&item.platform).to_string();
        if item.platform != "rss" {
            item.handle = item.handle.to_ascii_lowercase();
        }
        item.label = text::chars_at_most(item.label.trim(), 80);
        item.posts = scrub_posts(std::mem::take(&mut item.posts), &keywords, &item.platform);
        item.seen = scrub_seen(std::mem::take(&mut item.seen));
    }
    if profile.cache.len() > MAX_CACHE {
        let extra = profile.cache.len() - MAX_CACHE;
        profile.cache.drain(0..extra);
    }
    profile
}

fn scrub_board(board: &mut Board, platform: &str, keywords: &[String]) {
    for card in &mut board.cards {
        scrub_card(card, platform, keywords);
    }
    board.cards.retain(|card| !card.handle.is_empty());
    if board.cards.len() > MAX_CARDS {
        board.cards.truncate(MAX_CARDS);
    }
}

/// Moves folder lists off individual boards onto the profile, then drops
/// assignments that no longer name a folder.
fn lift_folders(profile: &mut Profile) {
    let mut incoming = Vec::new();
    for board in [
        &mut profile.x,
        &mut profile.threads,
        &mut profile.instagram,
        &mut profile.weibo,
        &mut profile.rss,
    ] {
        incoming.append(&mut board.folders);
    }
    for folder in incoming {
        let id = folder.id.trim();
        let name = folder.name.trim();
        if id.is_empty() || name.is_empty() {
            continue;
        }
        if profile.folders.iter().any(|have| have.id == id) {
            continue;
        }
        if profile.folders.len() >= MAX_FOLDERS {
            break;
        }
        profile.folders.push(RssFolder {
            id: id.to_string(),
            name: text::chars_at_most(name, 40),
        });
    }
    let ids: std::collections::HashSet<String> = profile.folders.iter().map(|folder| folder.id.clone()).collect();
    for board in [
        &mut profile.x,
        &mut profile.threads,
        &mut profile.instagram,
        &mut profile.weibo,
        &mut profile.rss,
    ] {
        for card in &mut board.cards {
            if !card.folder.is_empty() && !ids.contains(&card.folder) {
                card.folder.clear();
            }
        }
    }
}

fn scrub_card(card: &mut Card, platform: &str, keywords: &[String]) {
    if !card.handle.is_empty() && !source::valid_handle(platform, &card.handle) {
        *card = Card::default();
        return;
    }
    if platform != "rss" {
        card.handle = card.handle.to_ascii_lowercase();
    }
    card.label = text::chars_at_most(card.label.trim(), 80);
    card.note = text::chars_at_most(card.note.trim(), 180);
    card.posts = Arc::new(scrub_posts(take_posts(&mut card.posts), keywords, platform));
    card.seen = scrub_seen(std::mem::take(&mut card.seen));
}

pub fn parse_keywords(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for piece in raw.split([',', '\n', '，']) {
        let trimmed = piece.trim();
        if trimmed.is_empty() || trimmed.chars().count() > 80 {
            continue;
        }
        if out
            .iter()
            .any(|have: &String| have.eq_ignore_ascii_case(trimmed))
        {
            continue;
        }
        out.push(trimmed.to_string());
        if out.len() == 40 {
            break;
        }
    }
    out
}

/// First launch of the mixed stream: an older file has no `home`, so it opens on 全部.
fn migrate_timeline_home(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let has_home = obj
        .get("settings")
        .and_then(|settings| settings.get("home"))
        .and_then(|home| home.as_str())
        .is_some();
    if has_home {
        return;
    }
    obj.insert("focus".into(), serde_json::Value::String("all".into()));
    let settings = obj
        .entry("settings")
        .or_insert_with(|| serde_json::json!({}));
    if let Some(map) = settings.as_object_mut() {
        map.insert("home".into(), serde_json::Value::String("timeline".into()));
    }
}

pub fn set_home(mut profile: Profile, home: &str) -> Result<Profile, String> {
    match home.trim() {
        "timeline" | "board" => {
            profile.settings.home = home.trim().to_string();
            Ok(profile)
        }
        _ => Err("首页只能是动态或看板。".into()),
    }
}

pub fn set_focus(mut profile: Profile, focus: &str) -> Result<Profile, String> {
    let Some(focus) = canon_focus_strict(focus) else {
        return Err("没有这个平台。".into());
    };
    profile.focus = focus.to_string();
    Ok(profile)
}

pub fn set_refresh(mut profile: Profile, seconds: u32) -> Result<Profile, String> {
    let seconds = u64::from(seconds);
    if !(MIN_REFRESH_SECS..=MAX_REFRESH_SECS).contains(&seconds) {
        return Err("刷新间隔要在 30 秒和 60 分钟之间。".into());
    }
    profile.settings.refresh_secs = seconds;
    Ok(profile)
}

pub fn set_notify(mut profile: Profile, on: bool) -> Profile {
    profile.settings.notify = on;
    profile
}

pub fn set_notify_platform(mut profile: Profile, platform: &str, on: bool) -> Result<Profile, String> {
    if super::notice::set_platform(&mut profile.settings, platform, on) {
        Ok(profile)
    } else {
        Err("没有这个平台。".into())
    }
}

pub fn set_translate(
    mut profile: Profile,
    lang: &str,
    url: &str,
    token: &str,
    model: &str,
) -> Result<Profile, String> {
    let lang = super::translate::canon_lang(lang)?;
    let url = super::translate::normalize_base(url);
    if !url.is_empty() && !super::translate::endpoint_ok(&url) {
        return Err("接口地址要是 https，或本机的 http://127.0.0.1。".into());
    }
    if url.chars().count() > 300 {
        return Err("接口地址太长了。".into());
    }
    let model = super::translate::canon_model(model)?;
    profile.settings.translate_lang = lang;
    profile.settings.translate_url = url;
    profile.settings.translate_token = token.trim().chars().filter(|char| !char.is_control()).take(200).collect();
    profile.settings.translate_model = model;
    Ok(profile)
}

pub fn set_theme(mut profile: Profile, theme: &str) -> Result<Profile, String> {
    let Some(theme) = canon_theme_strict(theme) else {
        return Err("外观只能是浅色、深色，或跟随系统。".into());
    };
    profile.settings.theme = theme.into();
    Ok(profile)
}

pub fn set_mute(mut profile: Profile, raw: &str) -> Profile {
    profile.settings.mute_keywords = parse_keywords(raw);
    let keywords = profile.settings.mute_keywords.clone();
    for platform in ["x", "threads", "instagram", "weibo", "rss"] {
        for card in &mut board_mut(&mut profile, platform).cards {
            Arc::make_mut(&mut card.posts).retain(|post| !muted(post, &keywords));
        }
    }
    for item in &mut profile.cache {
        item.posts.retain(|post| !muted(post, &keywords));
    }
    profile
}

pub fn set_handle(
    mut profile: Profile,
    platform: &str,
    index: usize,
    input: &str,
) -> Result<Profile, String> {
    set_handle_in(&mut profile, platform, index, input)?;
    Ok(profile)
}

pub fn set_handle_in(
    profile: &mut Profile,
    platform: &str,
    index: usize,
    input: &str,
) -> Result<(), String> {
    let Some(platform) = named_platform(platform) else {
        return Err("先选一个平台，再添加账号。".into());
    };
    let len = board(&profile, platform).cards.len();
    if index > len {
        return Err("没有这个格子。".into());
    }
    let previous = if index == len {
        Card::default()
    } else {
        board(&profile, platform).cards[index].clone()
    };
    let input = input.trim();
    if input.is_empty() {
        if index == len {
            return Ok(());
        }
        park(profile, platform, &previous);
        board_mut(profile, platform).cards.remove(index);
        return Ok(());
    }
    if index == len && len >= MAX_CARDS {
        return Err("这个列表已经太长了。".into());
    }
    let hit = source::parse_source_for(input, platform);
    if !hit.supported {
        return Err(hit.message);
    }
    if hit.kind != platform {
        return Err("这个账号属于另一个平台。先切到那个平台再添加。".into());
    }
    if board(&profile, platform)
        .cards
        .iter()
        .enumerate()
        .any(|(slot, card)| slot != index && card.handle.eq_ignore_ascii_case(&hit.handle))
    {
        return Err("这个账号已经在这个列表里。".into());
    }
    assign_hit(profile, platform, index, &previous, hit);
    Ok(())
}

/// Parses an RSS or Atom document into the feed card and merges entries.
pub fn ingest_rss(profile: &mut Profile, index: usize, xml: &str) -> Result<bool, String> {
    if index >= profile.rss.cards.len() || profile.rss.cards[index].handle.is_empty() {
        return Err("没有这个订阅。".into());
    }
    let handle = profile.rss.cards[index].handle.clone();
    let parsed = match super::rss::parse_feed(xml) {
        Ok(feed) => feed,
        Err(error) => {
            profile.rss.cards[index].last_attempt_ms = unix_ms();
            profile.rss.cards[index].fail_streak = profile.rss.cards[index].fail_streak.saturating_add(1);
            profile.rss.cards[index].note = error;
            eprintln!("[pulse] rss fetch handle={handle} posts=0");
            return Ok(true);
        }
    };
    if profile.rss.cards[index].label.trim().is_empty() && !parsed.title.is_empty() {
        profile.rss.cards[index].label = text::chars_at_most(parsed.title.trim(), 80);
    }
    if profile.rss.cards[index].avatar.is_empty() && parsed.image.starts_with("https://") {
        profile.rss.cards[index].avatar = parsed.image;
    }
    let known_ids: Vec<String> = profile.rss.cards[index]
        .posts
        .iter()
        .map(|post| post.id.clone())
        .collect();
    let posts = parsed.posts.len();
    let keywords = profile.settings.mute_keywords.clone();
    let merged = merge_posts(
        take_posts(&mut profile.rss.cards[index].posts),
        parsed.posts,
        &keywords,
        "rss",
    );
    let settings = profile.settings.clone();
    let card = &mut profile.rss.cards[index];
    card.posts = Arc::new(merged);
    card.last_fetch_ms = unix_ms();
    card.last_attempt_ms = card.last_fetch_ms;
    card.fail_streak = 0;
    card.note.clear();
    if let Some(notice) = super::notice::consider_known(card, "rss", &settings, &known_ids) {
        crate::notify::deliver(&notice);
    }
    eprintln!("[pulse] rss fetch handle={handle} posts={posts}");
    Ok(true)
}

pub fn import_opml(profile: &mut Profile, xml: &str) -> Result<u32, String> {
    let feeds = super::rss::parse_opml(xml)?;
    let mut changed = 0u32;
    for feed in feeds {
        let folder = folder_for_name(profile, &feed.folder).unwrap_or_default();
        if let Some(card) = profile
            .rss
            .cards
            .iter_mut()
            .find(|card| card.handle.eq_ignore_ascii_case(&feed.url))
        {
            if card.folder != folder {
                card.folder = folder;
                changed += 1;
            }
            continue;
        }
        if profile.rss.cards.len() >= MAX_CARDS {
            break;
        }
        let mut card = Card {
            handle: feed.url,
            folder,
            ..Card::default()
        };
        if !feed.title.is_empty() {
            card.label = text::chars_at_most(feed.title.trim(), 80);
        }
        profile.rss.cards.push(card);
        changed += 1;
    }
    if changed == 0 {
        return Err("这些订阅都已经在列表里。".into());
    }
    Ok(changed)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RssEntry {
    pub index: u32,
    pub title: String,
    pub host: String,
    pub url: String,
    pub posts: u32,
    pub empty: bool,
    pub fetched: bool,
    pub note: String,
    /// Folder id. Empty means 未分类.
    pub folder: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RssCatalog {
    pub folders: Vec<RssFolder>,
    pub feeds: Vec<RssEntry>,
}

/// Folders plus one row per RSS card, without article bodies.
pub fn rss_catalog(profile: &Profile) -> RssCatalog {
    let feeds = profile
        .rss
        .cards
        .iter()
        .enumerate()
        .filter(|(_, card)| !card.handle.is_empty())
        .map(|(index, card)| {
            let host = feed_host(&card.handle);
            let title = if card.label.trim().is_empty() {
                host.clone()
            } else {
                card.label.clone()
            };
            RssEntry {
                index: index as u32,
                title,
                host,
                url: card.handle.clone(),
                posts: card.posts.len() as u32,
                empty: card.posts.is_empty(),
                fetched: card.last_fetch_ms > 0,
                note: card.note.clone(),
                folder: card.folder.clone(),
            }
        })
        .collect();
    RssCatalog {
        folders: profile.folders.clone(),
        feeds,
    }
}

/// The folder id for `name`, creating it when the name is new.
pub fn folder_for_name(profile: &mut Profile, name: &str) -> Result<String, String> {
    let name = clean_folder_name(name)?;
    if let Some(folder) = profile.folders.iter().find(|folder| folder.name.eq_ignore_ascii_case(&name)) {
        return Ok(folder.id.clone());
    }
    create_rss_folder(profile, &name)
}

pub fn create_rss_folder(profile: &mut Profile, name: &str) -> Result<String, String> {
    let name = clean_folder_name(name)?;
    if name == "未分类" {
        return Err("未分类是没有放进文件夹的订阅。".into());
    }
    if profile.folders.iter().any(|folder| folder.name.eq_ignore_ascii_case(&name)) {
        return Err("已经有这个文件夹。".into());
    }
    if profile.folders.len() >= MAX_FOLDERS {
        return Err("文件夹太多了。".into());
    }
    let id = next_folder_id(&profile.folders);
    profile.folders.push(RssFolder { id: id.clone(), name });
    Ok(id)
}

pub fn rename_rss_folder(profile: &mut Profile, id: &str, name: &str) -> Result<(), String> {
    let name = clean_folder_name(name)?;
    if name == "未分类" {
        return Err("未分类是没有放进文件夹的订阅。".into());
    }
    if !profile.folders.iter().any(|folder| folder.id == id) {
        return Err("没有这个文件夹。".into());
    }
    if profile
        .folders
        .iter()
        .any(|folder| folder.id != id && folder.name.eq_ignore_ascii_case(&name))
    {
        return Err("已经有这个文件夹。".into());
    }
    if let Some(folder) = profile.folders.iter_mut().find(|folder| folder.id == id) {
        folder.name = name;
    }
    Ok(())
}

/// Removes the folder. Accounts and feeds in it stay, under 未分类.
pub fn delete_rss_folder(profile: &mut Profile, id: &str) -> Result<(), String> {
    let before = profile.folders.len();
    profile.folders.retain(|folder| folder.id != id);
    if profile.folders.len() == before {
        return Err("没有这个文件夹。".into());
    }
    for board in [
        &mut profile.x,
        &mut profile.threads,
        &mut profile.instagram,
        &mut profile.weibo,
        &mut profile.rss,
    ] {
        for card in &mut board.cards {
            if card.folder == id {
                card.folder.clear();
            }
        }
    }
    Ok(())
}

/// Puts one account or feed into a shared folder. Empty `folder_id` is 未分类.
pub fn move_card_folder(profile: &mut Profile, platform: &str, handle: &str, folder_id: &str) -> Result<(), String> {
    let Some(platform) = named_platform(platform) else {
        return Err("没有这个平台。".into());
    };
    let folder_id = folder_id.trim();
    if !folder_id.is_empty() && !profile.folders.iter().any(|folder| folder.id == folder_id) {
        return Err("没有这个文件夹。".into());
    }
    let handle = handle.trim();
    let Some(card) = board_mut(profile, platform)
        .cards
        .iter_mut()
        .find(|card| card.handle.eq_ignore_ascii_case(handle))
    else {
        return Err("没有这个账号。".into());
    };
    if card.folder == folder_id {
        return Err("已经在这个文件夹里。".into());
    }
    card.folder = folder_id.to_string();
    Ok(())
}

/// Moves feeds into `folder_id`. Empty means 未分类.
pub fn move_rss_urls(profile: &mut Profile, urls: &[String], folder_id: &str) -> Result<u32, String> {
    let folder_id = folder_id.trim();
    if !folder_id.is_empty() && !profile.folders.iter().any(|folder| folder.id == folder_id) {
        return Err("没有这个文件夹。".into());
    }
    let want: std::collections::HashSet<String> = urls
        .iter()
        .map(|url| url.trim().to_ascii_lowercase())
        .filter(|url| !url.is_empty())
        .collect();
    if want.is_empty() {
        return Err("先选要移动的订阅。".into());
    }
    let mut moved = 0u32;
    for card in &mut profile.rss.cards {
        if !want.contains(&card.handle.to_ascii_lowercase()) || card.folder == folder_id {
            continue;
        }
        card.folder = folder_id.to_string();
        moved += 1;
    }
    if moved == 0 {
        return Err("这些订阅已经在那里了。".into());
    }
    Ok(moved)
}

fn clean_folder_name(name: &str) -> Result<String, String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = text::chars_at_most(name.trim(), 40);
    if name.is_empty() {
        return Err("先写文件夹的名字。".into());
    }
    if name.contains('<') || name.contains('>') {
        return Err("文件夹名字不能带尖括号。".into());
    }
    Ok(name)
}

fn next_folder_id(folders: &[RssFolder]) -> String {
    let next = folders
        .iter()
        .filter_map(|folder| folder.id.strip_prefix('f')?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    format!("f{next}")
}

fn rss_in_folder(card: &Card, folder: &str) -> bool {
    if folder.is_empty() {
        return true;
    }
    if folder == "-" {
        return card.folder.is_empty();
    }
    card.folder == folder
}

/// Drops the feeds whose URL is in `urls`. Indexes are not used, so a second
/// tap cannot remove the card that slid into a deleted slot.
pub fn remove_rss_urls(profile: &mut Profile, urls: &[String]) -> u32 {
    let want: std::collections::HashSet<String> = urls
        .iter()
        .map(|url| url.trim().to_ascii_lowercase())
        .filter(|url| !url.is_empty())
        .collect();
    if want.is_empty() {
        return 0;
    }
    let cards = std::mem::take(&mut profile.rss.cards);
    let mut keep = Vec::with_capacity(cards.len());
    let mut removed = 0u32;
    for card in cards {
        if want.contains(&card.handle.to_ascii_lowercase()) {
            park(profile, "rss", &card);
            removed += 1;
        } else {
            keep.push(card);
        }
    }
    profile.rss.cards = keep;
    removed
}

/// Drops every RSS card that has no stored posts, including one that never fetched.
pub fn clear_empty_rss(profile: &mut Profile) -> u32 {
    let urls: Vec<String> = profile
        .rss
        .cards
        .iter()
        .filter(|card| !card.handle.is_empty() && card.posts.is_empty())
        .map(|card| card.handle.clone())
        .collect();
    remove_rss_urls(profile, &urls)
}

fn feed_host(url: &str) -> String {
    let rest = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.trim_start_matches("www.");
    if host.is_empty() {
        url.trim().to_string()
    } else {
        host.to_string()
    }
}

pub fn export_opml(profile: &Profile) -> String {
    let names: std::collections::HashMap<&str, &str> = profile
        .folders
        .iter()
        .map(|folder| (folder.id.as_str(), folder.name.as_str()))
        .collect();
    let feeds: Vec<(String, String, String)> = profile
        .rss
        .cards
        .iter()
        .filter(|card| !card.handle.is_empty())
        .map(|card| {
            let title = if card.label.trim().is_empty() {
                card.handle.clone()
            } else {
                card.label.clone()
            };
            let folder = names.get(card.folder.as_str()).copied().unwrap_or("").to_string();
            (title, card.handle.clone(), folder)
        })
        .collect();
    super::rss::write_opml(&feeds)
}

/// Stores a resolved Weibo uid on that platform's list, with the screen name and photo when known.
pub fn add_weibo(
    profile: &mut Profile,
    index: usize,
    uid: &str,
    label: &str,
    avatar: &str,
) -> Result<(), String> {
    set_handle_in(profile, "weibo", index, uid)?;
    let Some(card) = board_mut(profile, "weibo").cards.get_mut(index) else {
        return Err("没有这个格子。".into());
    };
    if !label.trim().is_empty() {
        card.label = text::chars_at_most(label.trim(), 80);
    }
    let avatar = super::weibo::https_portrait(avatar.trim());
    if avatar.starts_with("https://") && avatar.len() < 500 {
        card.avatar = avatar;
    }
    Ok(())
}

fn assign_hit(
    profile: &mut Profile,
    platform: &str,
    index: usize,
    previous: &Card,
    hit: source::SourceHit,
) {
    if index < board(profile, platform).cards.len() && previous.handle.eq_ignore_ascii_case(&hit.handle)
    {
        board_mut(profile, platform).cards[index].handle = hit.handle;
        return;
    }
    let restored = take_cache(profile, platform, &hit.handle);
    park(profile, platform, previous);
    let card = if let Some(cached) = restored {
        Card {
            handle: hit.handle,
            label: cached.label,
            avatar: cached.avatar,
            posts: Arc::new(cached.posts),
            seen: cached.seen,
            last_fetch_ms: cached.last_fetch_ms,
            last_attempt_ms: cached.last_attempt_ms,
            fail_streak: cached.fail_streak,
            notify_ms: cached.notify_ms,
            note: String::new(),
            folder: if platform == "rss" { previous.folder.clone() } else { String::new() },
        }
    } else {
        Card {
            handle: hit.handle,
            ..Card::default()
        }
    };
    let cards = &mut board_mut(profile, platform).cards;
    if index == cards.len() {
        cards.push(card);
    } else {
        cards[index] = card;
    }
}

/// Moves a card one place up (`direction` negative) or down. The ends stay put.
pub fn move_card(
    mut profile: Profile,
    platform: &str,
    index: usize,
    direction: i32,
) -> Result<Profile, String> {
    let Some(platform) = named_platform(platform) else {
        return Err("没有这个平台。".into());
    };
    if direction == 0 {
        return Ok(profile);
    }
    let cards = &mut board_mut(&mut profile, platform).cards;
    let Some(next) = index.checked_add_signed(direction as isize) else {
        return Ok(profile);
    };
    if index >= cards.len() || next >= cards.len() {
        return Ok(profile);
    }
    cards.swap(index, next);
    Ok(profile)
}

pub fn ingest(
    mut profile: Profile,
    platform: &str,
    index: usize,
    message: &str,
) -> Result<(Profile, bool), String> {
    let Some(platform) = named_platform(platform) else {
        return Err("没有这个平台。".into());
    };
    if index >= board(&profile, platform).cards.len() {
        return Err("没有这个格子。".into());
    }
    let card_handle = board(&profile, platform).cards[index].handle.clone();
    if card_handle.is_empty() {
        return Err("这个格子还没有账号。".into());
    }
    let incoming: Incoming =
        serde_json::from_str(message).map_err(|_| "页面消息无法读取。".to_string())?;
    if !incoming.handle.is_empty() && !incoming.handle.eq_ignore_ascii_case(&card_handle) {
        return Ok((profile, false));
    }
    if !incoming.detail.is_empty() {
        eprintln!("[pulse] {}", incoming.detail);
    }
    let found_images: usize = incoming.posts.iter().map(|post| post.images.len()).sum();
    let inlined = remember_inlines(&incoming.inlines);
    if !incoming.avatar.trim().is_empty() {
        let avatar = clean_avatar(&incoming.avatar);
        if !avatar.is_empty() {
            board_mut(&mut profile, platform).cards[index].avatar = avatar;
        }
    }
    if !incoming.display_name.trim().is_empty() {
        board_mut(&mut profile, platform).cards[index].label =
            text::chars_at_most(incoming.display_name.trim(), 80);
    }
    let kind = incoming.kind.to_ascii_lowercase();
    if kind == "ig-api" {
        return finish_api(profile, platform, index, &card_handle, &incoming, true);
    }
    if kind == "weibo-api" {
        return finish_api(profile, platform, index, &card_handle, &incoming, false);
    }
    if kind == "embedded" {
        return finish_embedded(profile, platform, index, incoming);
    }
    if kind == "images" {
        eprintln!(
            "[pulse] images card={platform}:{index} found={found_images} inlined={inlined}"
        );
        return Ok((profile, true));
    }
    let state = incoming.state.to_ascii_lowercase();
    if kind == "session" && state == "signed-in" {
        // Login is recorded by the screen. The card already has its posts.
        return Ok((profile, false));
    }
    if kind == "session" && state == "signed-out" {
        // A Weibo guest still receives the public timeline. The screen marks
        // the account signed out; the card keeps the posts it already has.
        if source::canon_platform(platform) == "weibo" {
            return Ok((profile, false));
        }
        board_mut(&mut profile, platform).cards[index].note = login_note(platform);
        return Ok((profile, true));
    }
    if kind == "session" && state == "blocked" {
        board_mut(&mut profile, platform).cards[index].note = blocked_note(platform);
        return Ok((profile, true));
    }
    let keywords = profile.settings.mute_keywords.clone();
    let incoming_posts: Vec<Post> = incoming
        .posts
        .into_iter()
        .map(IncomingPost::into_post)
        .collect();
    let parsed = scrub_posts(incoming_posts.clone(), &keywords, platform);
    remember_fetch(platform, index, &card_handle, &parsed);
    let known_ids: Vec<String> = board(&profile, platform).cards[index]
        .posts
        .iter()
        .map(|post| post.id.clone())
        .collect();
    let merged = merge_posts(
        take_posts(&mut board_mut(&mut profile, platform).cards[index].posts),
        incoming_posts,
        &keywords,
        platform,
    );
    let empty = merged.is_empty();
    let kept_images: usize = merged.iter().map(|post| post.images.len()).sum();
    eprintln!(
        "[pulse] images card={platform}:{index} found={found_images} kept={kept_images} inlined={inlined}"
    );
    let settings = profile.settings.clone();
    let card = &mut board_mut(&mut profile, platform).cards[index];
    card.posts = Arc::new(merged);
    // An empty or failed read must not look like a finished refresh.
    // The scheduler would otherwise wait the full interval and keep the old posts.
    if !parsed.is_empty() {
        card.last_fetch_ms = unix_ms();
    }
    if let Some(notice) = if known_ids.is_empty() {
        super::notice::consider_card(card, platform, &settings)
    } else {
        super::notice::consider_known(card, platform, &settings, &known_ids)
    } {
        crate::notify::deliver(&notice);
    }
    card.note = if empty && state == "empty" {
        "这次没读到帖子。".into()
    } else if empty && state == "blocked" {
        blocked_note(platform)
    } else {
        String::new()
    };
    Ok((profile, true))
}

fn finish_embedded(
    mut profile: Profile,
    platform: &str,
    index: usize,
    incoming: Incoming,
) -> Result<(Profile, bool), String> {
    let handle = board(&profile, platform).cards[index].handle.clone();
    let dom_posts = incoming.posts.len();
    let matched = incoming.scripts.len();
    let parsed = super::embed::parse_scripts(&incoming.scripts, platform, &handle);
    let json_posts = parsed.posts.len();
    let json_images: usize = parsed.posts.iter().map(|post| post.images.len()).sum();
    if !parsed.display_name.is_empty() {
        board_mut(&mut profile, platform).cards[index].label =
            text::chars_at_most(parsed.display_name.trim(), 80);
    }
    if !parsed.avatar.is_empty() {
        let avatar = clean_avatar(&parsed.avatar);
        if !avatar.is_empty() {
            board_mut(&mut profile, platform).cards[index].avatar = avatar;
        }
    }
    if let Some(path) = write_dump(platform, &handle, &incoming.html) {
        eprintln!(
            "[pulse] dump path={} sjs={} matched={} jsonPosts={} domPosts={}",
            path.display(),
            incoming.sjs_count,
            matched,
            json_posts,
            dom_posts
        );
    } else {
        eprintln!(
            "[pulse] sjs={} matched={matched} jsonPosts={json_posts} domPosts={dom_posts} images={json_images}",
            incoming.sjs_count
        );
    }
    if json_posts > 0 {
        let keywords = profile.settings.mute_keywords.clone();
        let posts = scrub_posts(
            parsed
                .posts
                .into_iter()
                .map(|post| Post {
                    id: post.id,
                    text: post.text,
                    time_ms: post.time_ms,
                    url: post.url,
                    images: post.images,
                    link_title: String::new(),
                    link_url: String::new(),
                    quote: None,
                })
                .collect(),
            &keywords,
            platform,
        );
        let got_posts = !posts.is_empty();
        remember_fetch(platform, index, &handle, &posts);
        let settings = profile.settings.clone();
        let known_ids: Vec<String> = board(&profile, platform).cards[index]
            .posts
            .iter()
            .map(|post| post.id.clone())
            .collect();
        let merged = merge_posts(
            take_posts(&mut board_mut(&mut profile, platform).cards[index].posts),
            posts,
            &keywords,
            platform,
        );
        let card = &mut board_mut(&mut profile, platform).cards[index];
        card.posts = Arc::new(merged);
        if got_posts {
            card.last_fetch_ms = unix_ms();
        }
        if let Some(notice) = super::notice::consider_known(card, platform, &settings, &known_ids) {
            crate::notify::deliver(&notice);
        }
        card.note.clear();
        return Ok((profile, true));
    }
    if dom_posts == 0 {
        return Ok((profile, true));
    }
    let message = serde_json::json!({
        "type": "posts",
        "handle": handle,
        "displayName": incoming.display_name,
        "avatar": incoming.avatar,
        "posts": incoming.posts,
    });
    ingest(profile, platform, index, &message.to_string())
}

fn write_dump(platform: &str, handle: &str, html: &str) -> Option<std::path::PathBuf> {
    if html.is_empty() || !super::embed::dump_enabled() {
        return None;
    }
    let home = std::env::var_os("HOME")?;
    match super::embed::dump_file(std::path::Path::new(&home), platform, handle, html) {
        Ok(path) => Some(path),
        Err(err) => {
            eprintln!("[pulse] dump failed {err}");
            None
        }
    }
}

/// Marks one post read on its card. Already-read and unknown ids change nothing.
pub fn mark_post_read(profile: &mut Profile, platform: &str, index: usize, id: &str) -> bool {
    let Some(platform) = named_platform(platform) else {
        return false;
    };
    let Some(card) = board_mut(profile, platform).cards.get_mut(index) else {
        return false;
    };
    if !card.posts.iter().any(|post| post.id == id) || card.seen.iter().any(|have| have == id) {
        return false;
    }
    push_seen(&mut card.seen, id);
    card.seen.iter().any(|have| have == id)
}

pub fn star_key(platform: &str, id: &str) -> String {
    format!("{}:{id}", source::canon_platform(platform))
}

pub fn is_starred(profile: &Profile, platform: &str, id: &str) -> bool {
    let key = star_key(platform, id);
    profile.stars.iter().any(|star| star_key(&star.platform, &star.id) == key)
}

/// Stars the post, or removes the star if it is already there.
pub fn toggle_star(profile: &mut Profile, platform: &str, id: &str) -> Result<bool, String> {
    let Some(platform) = named_platform(platform) else {
        return Err("没有这个平台。".into());
    };
    let key = star_key(platform, id);
    if let Some(index) = profile
        .stars
        .iter()
        .position(|star| star_key(&star.platform, &star.id) == key)
    {
        profile.stars.remove(index);
        return Ok(false);
    }
    let snapshot = {
        let Some((card, post)) = find_post(profile, platform, id) else {
            return Err("这篇不在列表里。".into());
        };
        let author = if card.label.trim().is_empty() {
            format!("@{}", card.handle)
        } else {
            card.label.trim().to_string()
        };
        Star {
            id: post.id.clone(),
            platform: platform.to_string(),
            handle: card.handle.clone(),
            author: text::chars_at_most(&author, 80),
            title: text::chars_at_most(post.link_title.trim(), 200),
            body: text::chars_at_most(post.text.trim(), 2000),
            url: if post.url.starts_with("http") { post.url.clone() } else { String::new() },
            starred_ms: unix_ms() as i64,
        }
    };
    profile.stars.push(snapshot);
    if profile.stars.len() > MAX_STARS {
        let extra = profile.stars.len() - MAX_STARS;
        profile.stars.drain(0..extra);
    }
    Ok(true)
}

fn find_post<'a>(profile: &'a Profile, platform: &str, id: &str) -> Option<(&'a Card, &'a Post)> {
    for card in &board(profile, platform).cards {
        if let Some(post) = card.posts.iter().find(|post| post.id == id) {
            return Some((card, post));
        }
    }
    None
}

pub fn mark_read(mut profile: Profile, platform: &str, index: usize) -> Result<Profile, String> {
    let Some(platform) = named_platform(platform) else {
        return Err("没有这个平台。".into());
    };
    let card = board_mut(&mut profile, platform)
        .cards
        .get_mut(index)
        .ok_or("没有这个格子。")?;
    for post in card.posts.iter() {
        push_seen(&mut card.seen, &post.id);
    }
    Ok(profile)
}

pub fn mark_all_read(mut profile: Profile) -> Profile {
    let slots = focus_slots(&profile);
    for slot in slots {
        if slot.platform.is_empty() {
            continue;
        }
        let ids: Vec<String> = board(&profile, &slot.platform).cards[slot.index as usize]
            .posts
            .iter()
            .map(|post| post.id.clone())
            .collect();
        let card = &mut board_mut(&mut profile, &slot.platform).cards[slot.index as usize];
        for id in ids {
            push_seen(&mut card.seen, &id);
        }
    }
    profile
}

/// Makes every card in the focused view due on the next tick.
pub fn request_refresh(mut profile: Profile) -> Profile {
    let slots = focus_slots(&profile);
    for slot in slots {
        if slot.platform.is_empty() {
            continue;
        }
        let card = &mut board_mut(&mut profile, &slot.platform).cards[slot.index as usize];
        if !card.handle.is_empty() {
            card.last_fetch_ms = 0;
            card.last_attempt_ms = 0;
            card.fail_streak = 0;
        }
    }
    profile
}

pub fn touch_fetch(
    mut profile: Profile,
    platform: &str,
    index: usize,
    now_ms: i64,
) -> Result<Profile, String> {
    let Some(platform) = named_platform(platform) else {
        return Err("没有这个平台。".into());
    };
    let card = board_mut(&mut profile, platform)
        .cards
        .get_mut(index)
        .ok_or("没有这个格子。")?;
    card.last_fetch_ms = now_ms.max(0) as u64;
    Ok(profile)
}

pub fn present_card(profile: &Profile, platform: &str, index: usize) -> Result<PresentCard, String> {
    let Some(platform) = named_platform(platform) else {
        return Err("没有这个平台。".into());
    };
    let card = board(profile, platform)
        .cards
        .get(index)
        .ok_or("没有这个格子。")?;
    let keywords = &profile.settings.mute_keywords;
    let posts: Vec<&Post> = card
        .posts
        .iter()
        .filter(|post| !muted(post, keywords))
        .collect();
    let unread = posts.iter().filter(|post| !seen(card, &post.id)).count() as u32;
    Ok(PresentCard {
        ok: true,
        error: String::new(),
        platform: platform.to_string(),
        handle: card.handle.clone(),
        label: card.label.clone(),
        avatar: card.avatar.clone(),
        url: if card.handle.is_empty() {
            String::new()
        } else {
            source::profile_url(platform, &card.handle).unwrap_or_default()
        },
        unread,
        note: card.note.clone(),
        last_fetch_ms: card.last_fetch_ms,
        posts: posts
            .into_iter()
            .map(|post| present_post(post, card, is_starred(profile, platform, &post.id)))
            .collect(),
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FocusSlot {
    pub platform: String,
    pub index: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UnreadTotals {
    pub x: u32,
    pub threads: u32,
    pub instagram: u32,
    pub weibo: u32,
    pub rss: u32,
    pub all: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineItem {
    pub key: String,
    pub platform: String,
    pub handle: String,
    pub author: String,
    /// Profile photo. Empty when the card has not seen one yet.
    pub avatar: String,
    pub card_index: u32,
    pub post: PresentPost,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelinePage {
    pub ok: bool,
    pub total: u32,
    pub offset: u32,
    pub items: Vec<TimelineItem>,
}

/// One page of the mixed stream, newest dated post first. Untimed posts follow.
/// Muted posts stay out. `focus` empty means the profile's current filter.
pub fn timeline(profile: &Profile, focus: &str, offset: usize, limit: usize, folder: &str) -> TimelinePage {
    let focus = if focus.trim().is_empty() {
        canon_focus(&profile.focus)
    } else {
        canon_focus(focus)
    };
    let platforms: &[&str] = if focus == "all" {
        &["x", "threads", "instagram", "weibo", "rss"]
    } else {
        match focus {
            "threads" => &["threads"],
            "instagram" => &["instagram"],
            "weibo" => &["weibo"],
            "rss" => &["rss"],
            _ => &["x"],
        }
    };
    let keywords = &profile.settings.mute_keywords;
    let folder = folder.trim();
    let mut rows: Vec<(i64, &str, usize, usize)> = Vec::new();
    for platform in platforms {
        for (index, card) in board(profile, platform).cards.iter().enumerate() {
            if card.handle.is_empty() {
                continue;
            }
            if !rss_in_folder(card, folder) {
                continue;
            }
            for (post_index, post) in card.posts.iter().enumerate() {
                if muted(post, keywords) {
                    continue;
                }
                rows.push((post.time_ms, platform, index, post_index));
            }
        }
    }
    rows.sort_by(|left, right| {
        time_key(left.0)
            .cmp(&time_key(right.0))
            .then(left.1.cmp(right.1))
            .then(left.2.cmp(&right.2))
            .then(left.3.cmp(&right.3))
    });
    let total = rows.len();
    let limit = limit.clamp(1, 80);
    let offset = offset.min(total);
    let items = rows
        .iter()
        .skip(offset)
        .take(limit)
        .map(|(_, platform, index, post_index)| {
            let card = &board(profile, platform).cards[*index];
            let post = &card.posts[*post_index];
            let author = stream_author(platform, card);
            TimelineItem {
                key: format!("{platform}:{}:{}", card.handle, post.id),
                platform: (*platform).to_string(),
                handle: card.handle.clone(),
                author,
                avatar: card.avatar.clone(),
                card_index: *index as u32,
                post: present_post(post, card, is_starred(profile, platform, &post.id)),
            }
        })
        .collect();
    TimelinePage {
        ok: true,
        total: total as u32,
        offset: offset as u32,
        items,
    }
}

/// Dated posts rank by recency. A missing time sorts after every dated post.
fn time_key(time_ms: i64) -> (u8, i64) {
    if time_ms > 0 {
        (0, -time_ms)
    } else {
        (1, 0)
    }
}

fn stream_author(platform: &str, card: &Card) -> String {
    if !card.label.trim().is_empty() {
        return card.label.clone();
    }
    if platform == "rss" {
        return feed_host(&card.handle);
    }
    if card.handle.is_empty() {
        return String::new();
    }
    format!("@{}", card.handle)
}

/// Accounts on the focused platform, or every account when focus is `all`.
pub fn focus_slots(profile: &Profile) -> Vec<FocusSlot> {
    let focus = canon_focus(&profile.focus);
    let platforms: &[&str] = if focus == "all" {
        &["x", "threads", "instagram", "weibo", "rss"]
    } else {
        match focus {
            "threads" => &["threads"],
            "instagram" => &["instagram"],
            "weibo" => &["weibo"],
            "rss" => &["rss"],
            _ => &["x"],
        }
    };
    let mut slots = Vec::new();
    for platform in platforms {
        for (index, card) in board(profile, platform).cards.iter().enumerate() {
            if card.handle.is_empty() {
                continue;
            }
            slots.push(FocusSlot {
                platform: (*platform).to_string(),
                index: index as u32,
            });
        }
    }
    slots
}

/// Unread posts on the focused list, without building the post bubbles.
pub fn focused_unread(profile: &Profile) -> u32 {
    focus_slots(profile)
        .iter()
        .map(|slot| card_unread(profile, &slot.platform, slot.index as usize))
        .sum()
}

/// Newest fetch time on the focused list.
pub fn focused_fresh_ms(profile: &Profile) -> u64 {
    focus_slots(profile)
        .iter()
        .filter_map(|slot| card_at(profile, &slot.platform, slot.index as usize))
        .map(|card| card.last_fetch_ms)
        .max()
        .unwrap_or(0)
}

/// Handle and post ids for the scroll clock. Image URLs stay out of this.
pub fn card_clock(profile: &Profile, platform: &str, index: usize) -> CardClock {
    let Some(card) = card_at(profile, platform, index) else {
        return CardClock::default();
    };
    let mut ids = String::new();
    for (at, post) in card.posts.iter().enumerate() {
        if at > 0 {
            ids.push(',');
        }
        ids.push_str(&post.id);
    }
    CardClock {
        handle: card.handle.clone(),
        last_fetch_ms: card.last_fetch_ms,
        last_attempt_ms: card.last_attempt_ms,
        ids,
    }
}

pub fn profile_url_at(profile: &Profile, platform: &str, index: usize) -> String {
    let Some(name) = named_platform(platform) else {
        return String::new();
    };
    let Some(card) = card_at(profile, name, index) else {
        return String::new();
    };
    if card.handle.is_empty() {
        return String::new();
    }
    source::profile_url(name, &card.handle).unwrap_or_default()
}

pub fn card_at<'a>(profile: &'a Profile, platform: &str, index: usize) -> Option<&'a Card> {
    let platform = named_platform(platform)?;
    board(profile, platform).cards.get(index)
}

fn card_unread(profile: &Profile, platform: &str, index: usize) -> u32 {
    let Some(card) = card_at(profile, platform, index) else {
        return 0;
    };
    card_badge(card, &profile.settings.mute_keywords)
}

pub fn card_badge(card: &Card, keywords: &[String]) -> u32 {
    card.posts
        .iter()
        .filter(|post| !muted(post, keywords) && !seen(card, &post.id))
        .count() as u32
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CardClock {
    pub handle: String,
    pub last_fetch_ms: u64,
    pub last_attempt_ms: u64,
    pub ids: String,
}

pub fn unread_totals(profile: &Profile) -> UnreadTotals {
    let x = board_unread(profile, "x");
    let threads = board_unread(profile, "threads");
    let instagram = board_unread(profile, "instagram");
    let weibo = board_unread(profile, "weibo");
    let rss = board_unread(profile, "rss");
    UnreadTotals {
        x,
        threads,
        instagram,
        weibo,
        rss,
        all: x
            .saturating_add(threads)
            .saturating_add(instagram)
            .saturating_add(weibo)
            .saturating_add(rss),
    }
}

fn board_unread(profile: &Profile, platform: &str) -> u32 {
    let keywords = &profile.settings.mute_keywords;
    board(profile, platform)
        .cards
        .iter()
        .map(|card| {
            card.posts
                .iter()
                .filter(|post| !muted(post, keywords) && !seen(card, &post.id))
                .count() as u32
        })
        .sum()
}

fn board<'a>(profile: &'a Profile, platform: &str) -> &'a Board {
    match platform {
        "threads" => &profile.threads,
        "instagram" => &profile.instagram,
        "weibo" => &profile.weibo,
        "rss" => &profile.rss,
        _ => &profile.x,
    }
}

fn board_mut<'a>(profile: &'a mut Profile, platform: &str) -> &'a mut Board {
    match platform {
        "threads" => &mut profile.threads,
        "instagram" => &mut profile.instagram,
        "weibo" => &mut profile.weibo,
        "rss" => &mut profile.rss,
        _ => &mut profile.x,
    }
}

fn named_platform(platform: &str) -> Option<&'static str> {
    match platform.trim().to_ascii_lowercase().as_str() {
        "x" => Some("x"),
        "threads" | "thread" => Some("threads"),
        "instagram" | "ig" => Some("instagram"),
        "weibo" | "wb" => Some("weibo"),
        "rss" | "feed" => Some("rss"),
        _ => None,
    }
}

pub fn canon_sync_mode(raw: &str) -> &'static str {
    match raw.trim() {
        "icloud" => "icloud",
        "https" => "https",
        _ => "off",
    }
}

pub fn set_sync_prefs(profile: &mut Profile, mode: &str, url: &str, token: &str, pull_on_open: bool) {
    profile.sync.mode = canon_sync_mode(mode).to_string();
    profile.sync.url = url.trim().to_string();
    profile.sync.token = token.trim().to_string();
    profile.sync.pull_on_open = pull_on_open;
}

pub fn canon_focus(raw: &str) -> &'static str {
    canon_focus_strict(raw).unwrap_or("x")
}

fn canon_focus_strict(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "x" => Some("x"),
        "threads" => Some("threads"),
        "instagram" => Some("instagram"),
        "weibo" => Some("weibo"),
        "rss" => Some("rss"),
        "all" => Some("all"),
        _ => None,
    }
}

fn present_post(post: &Post, card: &Card, starred: bool) -> PresentPost {
    PresentPost {
        id: post.id.clone(),
        segments: text::preview_segments(&post.text, PREVIEW_CHARS)
            .into_iter()
            .map(JsonSegment::from)
            .collect(),
        time_ms: post.time_ms,
        url: post.url.clone(),
        images: post.images.clone(),
        link_title: post.link_title.clone(),
        link_url: post.link_url.clone(),
        quote: post.quote.as_ref().map(|quote| PresentQuote {
            author: quote.author.clone(),
            url: quote.url.clone(),
            segments: text::preview_segments(&quote.text, 160)
                .into_iter()
                .map(JsonSegment::from)
                .collect(),
        }),
        unread: !seen(card, &post.id),
        starred,
    }
}

pub fn starred_timeline(profile: &Profile, offset: usize, limit: usize) -> TimelinePage {
    let mut stars: Vec<&Star> = profile.stars.iter().collect();
    stars.sort_by(|left, right| right.starred_ms.cmp(&left.starred_ms).then(left.id.cmp(&right.id)));
    let total = stars.len();
    let limit = limit.clamp(1, 80);
    let offset = offset.min(total);
    let items = stars
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|star| {
            let card_index = board(profile, &star.platform)
                .cards
                .iter()
                .position(|card| card.posts.iter().any(|post| post.id == star.id))
                .unwrap_or(0) as u32;
            TimelineItem {
                key: format!("star:{}:{}", star.platform, star.id),
                platform: star.platform.clone(),
                handle: star.handle.clone(),
                author: if star.author.is_empty() {
                    format!("@{}", star.handle)
                } else {
                    star.author.clone()
                },
                avatar: String::new(),
                card_index,
                post: PresentPost {
                    id: star.id.clone(),
                    segments: text::preview_segments(&star.body, PREVIEW_CHARS)
                        .into_iter()
                        .map(JsonSegment::from)
                        .collect(),
                    time_ms: star.starred_ms,
                    url: star.url.clone(),
                    images: Vec::new(),
                    link_title: star.title.clone(),
                    link_url: String::new(),
                    quote: None,
                    unread: false,
                    starred: true,
                },
            }
        })
        .collect();
    TimelinePage {
        ok: true,
        total: total as u32,
        offset: offset as u32,
        items,
    }
}

fn scrub_stars(profile: &mut Profile) {
    profile.stars.retain(|star| !star.id.trim().is_empty() && named_platform(&star.platform).is_some());
    for star in &mut profile.stars {
        star.platform = source::canon_platform(&star.platform).to_string();
        star.id = text::chars_at_most(star.id.trim(), 80);
        star.handle = text::chars_at_most(star.handle.trim(), 120);
        star.author = text::chars_at_most(star.author.trim(), 80);
        star.title = text::chars_at_most(star.title.trim(), 200);
        star.body = text::chars_at_most(star.body.trim(), 2000);
        if !star.url.starts_with("http") {
            star.url.clear();
        }
    }
    if profile.stars.len() > MAX_STARS {
        let extra = profile.stars.len() - MAX_STARS;
        profile.stars.drain(0..extra);
    }
    profile.briefing.script = text::chars_at_most(profile.briefing.script.trim(), 4000);
    profile.briefing.note = text::chars_at_most(profile.briefing.note.trim(), 200);
    profile.briefing.day = text::chars_at_most(profile.briefing.day.trim(), 16);
}

fn seen(card: &Card, id: &str) -> bool {
    card.seen.iter().any(|have| have == id)
}

fn canon_theme(theme: &str) -> &'static str {
    canon_theme_strict(theme).unwrap_or("system")
}

fn canon_theme_strict(theme: &str) -> Option<&'static str> {
    match theme.trim() {
        "light" => Some("light"),
        "dark" => Some("dark"),
        "system" => Some("system"),
        _ => None,
    }
}

fn park(profile: &mut Profile, platform: &str, card: &Card) {
    if card.handle.is_empty() {
        return;
    }
    profile.cache.retain(|item| {
        !(item.platform == platform && item.handle.eq_ignore_ascii_case(&card.handle))
    });
    profile.cache.push(CachedFeed {
        platform: platform.to_string(),
        handle: card.handle.clone(),
        label: card.label.clone(),
        avatar: card.avatar.clone(),
        posts: card.posts.as_ref().clone(),
        seen: card.seen.clone(),
        last_fetch_ms: card.last_fetch_ms,
        last_attempt_ms: card.last_attempt_ms,
        fail_streak: card.fail_streak,
        notify_ms: card.notify_ms,
    });
    if profile.cache.len() > MAX_CACHE {
        let extra = profile.cache.len() - MAX_CACHE;
        profile.cache.drain(0..extra);
    }
}

fn take_cache(profile: &mut Profile, platform: &str, handle: &str) -> Option<CachedFeed> {
    let position = profile
        .cache
        .iter()
        .position(|item| item.platform == platform && item.handle.eq_ignore_ascii_case(handle))?;
    Some(profile.cache.remove(position))
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

struct FetchNote {
    handle: String,
    newest_id: String,
    time_ms: i64,
    posts: usize,
    saw_posts: bool,
}

fn fetch_slot() -> &'static std::sync::Mutex<std::collections::HashMap<(String, usize), FetchNote>> {
    static SLOT: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<(String, usize), FetchNote>>,
    > = std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Remembers the best parse of the open fetch. Empty reads do not erase a
/// real one, and nothing is printed until [`finish_fetch`].
fn remember_fetch(platform: &str, index: usize, handle: &str, parsed: &[Post]) {
    let Ok(mut slots) = fetch_slot().lock() else {
        return;
    };
    let note = slots.entry((platform.to_string(), index)).or_insert(FetchNote {
        handle: handle.to_string(),
        newest_id: String::new(),
        time_ms: 0,
        posts: 0,
        saw_posts: false,
    });
    note.handle = handle.to_string();
    let Some(newest) = parsed
        .iter()
        .max_by(|a, b| a.time_ms.cmp(&b.time_ms).then_with(|| a.id.cmp(&b.id)))
    else {
        return;
    };
    note.newest_id = newest.id.clone();
    note.time_ms = newest.time_ms;
    note.posts = parsed.len();
    note.saw_posts = true;
}

pub fn format_fetch_line(platform: &str, handle: &str, newest_id: &str, time_ms: i64, posts: usize) -> String {
    if newest_id.is_empty() || posts == 0 {
        format!("[pulse] {platform} fetch handle={handle} newest=none time=0 posts=0")
    } else {
        format!("[pulse] {platform} fetch handle={handle} newest={newest_id} time={time_ms} posts={posts}")
    }
}

/// One log line for a profile fetch that just closed, and the card's retry clock.
/// A real parse clears the failure streak. An empty read backs off.
pub fn finish_fetch(profile: &mut Profile, platform: &str, index: usize) {
    let Some(platform) = named_platform(platform) else {
        return;
    };
    let handle = board(profile, platform)
        .cards
        .get(index)
        .map(|card| card.handle.clone())
        .unwrap_or_default();
    let note = fetch_slot()
        .lock()
        .ok()
        .and_then(|mut slots| slots.remove(&(platform.to_string(), index)));
    let saw = note.as_ref().is_some_and(|note| note.saw_posts);
    let (id, time_ms, posts, who) = match note {
        Some(note) if note.saw_posts => {
            let who = if handle.is_empty() { note.handle.clone() } else { handle };
            (note.newest_id, note.time_ms, note.posts, who)
        }
        Some(note) => {
            let who = if handle.is_empty() { note.handle } else { handle };
            (String::new(), 0, 0, who)
        }
        None => (String::new(), 0, 0, handle),
    };
    if let Some(card) = board_mut(profile, platform).cards.get_mut(index) {
        card.last_attempt_ms = unix_ms();
        if saw {
            card.fail_streak = 0;
        } else {
            card.fail_streak = card.fail_streak.saturating_add(1);
        }
    }
    eprintln!("{}", format_fetch_line(platform, &who, &id, time_ms, posts));
    if platform == "weibo" {
        eprintln!("[pulse] weibo fetch handle={who} posts={posts}");
    }
}

/// How long to wait after `streak` empty reads, capped at the refresh interval.
pub fn retry_backoff_ms(streak: u32, refresh_secs: u64) -> i64 {
    if streak == 0 {
        return 0;
    }
    let step = match streak {
        1 => 30_000,
        2 => 60_000,
        _ => 120_000,
    };
    let cap = refresh_secs.saturating_mul(1000) as i64;
    if cap == 0 { step } else { step.min(cap) }
}

fn merge_posts(
    existing: Vec<Post>,
    incoming: Vec<Post>,
    keywords: &[String],
    platform: &str,
) -> Vec<Post> {
    scrub_posts(
        existing.into_iter().chain(incoming).collect(),
        keywords,
        platform,
    )
}

fn take_posts(posts: &mut Arc<Vec<Post>>) -> Vec<Post> {
    std::mem::take(Arc::make_mut(posts))
}

fn scrub_posts(posts: Vec<Post>, keywords: &[String], platform: &str) -> Vec<Post> {
    let mut out: Vec<Post> = Vec::new();
    for post in posts {
        let Some(post) = scrub_post(post, platform) else {
            continue;
        };
        if muted(&post, keywords) {
            out.retain(|have| have.id != post.id);
            continue;
        }
        if let Some(existing) = out.iter_mut().find(|have| have.id == post.id) {
            *existing = post;
        } else {
            out.push(post);
        }
    }
    // Timed posts stay in time order. Unknown times (`0`) keep the order they
    // were read in, and sit after the dated ones. Sorting them by id would scramble a grid.
    out.sort_by(|a, b| time_rank(a).cmp(&time_rank(b)));
    let cap = if platform == "rss" { MAX_RSS_POSTS } else { MAX_POSTS };
    if out.len() > cap {
        let extra = out.len() - cap;
        out.drain(0..extra);
    }
    out
}

fn time_rank(post: &Post) -> (u8, i64) {
    if post.time_ms > 0 { (0, post.time_ms) } else { (1, 0) }
}

fn scrub_post(post: Post, platform: &str) -> Option<Post> {
    if platform == "rss" {
        return scrub_rss_post(post);
    }
    let id = clean_id(&post.id, &post.url)?;
    // A missing timestamp is unknown, not "now", and not a reason to drop the post.
    let mut text = text::chars_at_most(post.text.trim(), 2000);
    if clock_text_only(&text) {
        text.clear();
    }
    let (link_url, link_title) = decorate_link(&text, &post.link_url, &post.link_title);
    let mut images = Vec::new();
    for image in post.images {
        if let Some(thumb) = thumb(&image) {
            if !images.contains(&thumb) {
                images.push(thumb);
            }
        }
        if images.len() == 4 {
            break;
        }
    }
    if text.is_empty() && images.is_empty() && link_url.is_empty() && post.quote.is_none() {
        return None;
    }
    Some(Post {
        url: clean_status_url(&post.url, &id, platform),
        quote: post.quote.and_then(clean_quote),
        id,
        text,
        time_ms: post.time_ms,
        images,
        link_title,
        link_url,
    })
}

fn scrub_rss_post(post: Post) -> Option<Post> {
    let id = post.id.trim();
    if !(id.len() == 17 && id.starts_with('r') && id[1..].chars().all(|char| char.is_ascii_hexdigit())) {
        return None;
    }
    let url = if post.url.starts_with("http") && post.url.len() < 500 {
        post.url.trim().to_string()
    } else {
        String::new()
    };
    let text = text::chars_at_most(post.text.trim(), RSS_TEXT_CHARS);
    let mut images = Vec::new();
    for image in post.images {
        if image.starts_with("https://") && image.len() < 2000 && !images.contains(&image) {
            images.push(image);
        }
        if images.len() == 4 {
            break;
        }
    }
    let title = text::chars_at_most(post.link_title.trim(), 180);
    if text.is_empty() && images.is_empty() && title.is_empty() {
        return None;
    }
    Some(Post {
        id: id.to_string(),
        text,
        time_ms: post.time_ms.max(0),
        url: url.clone(),
        images,
        link_title: title,
        link_url: url,
        quote: None,
    })
}

fn clean_quote(quote: Quote) -> Option<Quote> {
    let text = text::chars_at_most(quote.text.trim(), 500);
    let author = quote
        .author
        .trim()
        .trim_start_matches('@')
        .to_ascii_lowercase();
    let author = if source::is_handle(&author) {
        author
    } else {
        String::new()
    };
    if text.is_empty() && author.is_empty() {
        return None;
    }
    let url = if quote.url.starts_with("https://")
        && quote.url.contains("/status/")
        && quote.url.len() < 300
    {
        quote
            .url
            .split(['?', '#'])
            .next()
            .unwrap_or(&quote.url)
            .to_string()
    } else {
        String::new()
    };
    Some(Quote { author, text, url })
}

fn decorate_link(text: &str, url: &str, title: &str) -> (String, String) {
    let mut url = url.trim().to_string();
    if !usable_link(&url) {
        url.clear();
    }
    if url.is_empty() {
        if let Some(found) = text::first_https(text) {
            if usable_link(&found) {
                url = found;
            }
        }
    }
    if url.is_empty() {
        return (String::new(), String::new());
    }
    let title = title.trim();
    let title = if title.is_empty() {
        text::host_label(&url)
    } else {
        text::chars_at_most(title, 120)
    };
    (url, title)
}

fn usable_link(url: &str) -> bool {
    url.starts_with("https://")
        && url.len() > "https://a.co".len()
        && url.len() < 400
        && !url.contains("/status/")
        && !url.contains("/post/")
        && !url.contains("/reel/")
        && !url.contains("/p/")
        && !url.contains("pbs.twimg.com")
        && !url.contains("cdninstagram.com")
        && !url.contains("fbcdn.net")
}

/// A caption that is only a relative time ("2d") or a date stamp ("10/23/26") is the
/// `<time>` label, not the post.
pub(crate) fn clock_text_only(text: &str) -> bool {
    relative_time_only(text) || date_stamp_only(text)
}

fn date_stamp_only(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 24 {
        return false;
    }
    let numeric: Vec<&str> = text.split(['/', '-', '.']).filter(|part| !part.is_empty()).collect();
    if numeric.len() == 3
        && numeric
            .iter()
            .all(|part| (1..=4).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_digit()))
        && text.chars().all(|char| char.is_ascii_digit() || matches!(char, '/' | '-' | '.'))
    {
        return true;
    }
    text.contains('月') && text.contains('日') && text.chars().filter(|char| char.is_ascii_digit()).count() >= 2
}

/// A caption that is only Threads' or Instagram's relative time ("2d", "6h") is not a post.
fn relative_time_only(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 16 {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    if matches!(lower.as_str(), "now" | "just now" | "today" | "昨天" | "刚刚") {
        return true;
    }
    let bytes = lower.as_bytes();
    let digits = bytes.iter().take_while(|byte| byte.is_ascii_digit()).count();
    if digits == 0 || digits == bytes.len() {
        return false;
    }
    let rest = lower[digits..].trim();
    matches!(
        rest,
        "s" | "m"
            | "h"
            | "d"
            | "w"
            | "y"
            | "sec"
            | "min"
            | "hr"
            | "hour"
            | "hours"
            | "day"
            | "days"
            | "week"
            | "weeks"
            | "秒"
            | "分"
            | "分钟"
            | "小时"
            | "天"
            | "周"
            | "星期"
            | "年"
    )
}

fn clean_avatar(url: &str) -> String {
    let url = url.split('#').next().unwrap_or(url).trim();
    if super::media::accept_url(url) {
        url.to_string()
    } else {
        String::new()
    }
}

fn remember_inlines(inlines: &[InlineImage]) -> usize {
    let mut kept = 0;
    for inline in inlines {
        if inline.data.starts_with("data:image/")
            && inline.data.len() > 32
            && inline.data.len() < 1_400_000
            && super::media::accept_url(&inline.url)
        {
            super::media::remember(inline.url.clone(), inline.data.clone());
            eprintln!(
                "[pulse] image inline bytes={} url_len={}",
                inline.data.len(),
                inline.url.len()
            );
            kept += 1;
        }
    }
    kept
}

fn thumb(url: &str) -> Option<String> {
    let url = url.split('#').next().unwrap_or(url).trim();
    // Signed Threads and Instagram URLs keep their query. Stripping it, or
    // cutting them off around 800–4000 characters, drops the photo.
    if url.starts_with("https://")
        && (16..=8_000).contains(&url.len())
        && (url.contains("cdninstagram.com")
            || url.contains(".fbcdn.net")
            || url.contains("scontent.")
            || url.contains("sinaimg.cn"))
    {
        return Some(url.to_string());
    }
    if !is_twimg_media(url) || url.len() > 800 {
        return None;
    }
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    let mut parts: Vec<&str> = query
        .split('&')
        .filter(|part| !part.is_empty() && !part.starts_with("name="))
        .collect();
    parts.push("name=small");
    Some(format!("{path}?{}", parts.join("&")))
}

fn is_twimg_media(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url);
    path.starts_with("https://pbs.twimg.com/media/")
        || path.starts_with("https://pbs.twimg.com/ext_tw_video_thumb/")
        || path.starts_with("https://pbs.twimg.com/amplify_video_thumb/")
        || path.starts_with("https://pbs.twimg.com/tweet_video_thumb/")
}

fn clean_id(id: &str, url: &str) -> Option<String> {
    let id = id.trim();
    if is_post_id(id) {
        return Some(id.to_string());
    }
    snowflake_in(url).or_else(|| shortcode_in(url))
}

fn clean_status_url(url: &str, id: &str, platform: &str) -> String {
    let url = url.trim().split(['?', '#']).next().unwrap_or(url);
    if url.starts_with("https://") && url.contains(id) && url.len() < 300 {
        return url.to_string();
    }
    match source::canon_platform(platform) {
        "threads" => format!("https://www.threads.net/post/{id}"),
        "instagram" => format!("https://www.instagram.com/p/{id}/"),
        "weibo" => format!("https://m.weibo.cn/detail/{id}"),
        _ => format!("https://x.com/i/status/{id}"),
    }
}

fn is_snowflake(id: &str) -> bool {
    (1..=22).contains(&id.len()) && id.chars().all(|char| char.is_ascii_digit())
}

fn is_shortcode(id: &str) -> bool {
    let count = id.len();
    (5..=32).contains(&count)
        && id.chars().any(|char| char.is_ascii_alphabetic())
        && id
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
}

fn is_post_id(id: &str) -> bool {
    is_snowflake(id) || is_shortcode(id)
}

fn shortcode_in(url: &str) -> Option<String> {
    for marker in ["/p/", "/reel/", "/post/", "/t/"] {
        if let Some(rest) = url.split(marker).nth(1) {
            let id: String = rest
                .chars()
                .take_while(|char| char.is_ascii_alphanumeric() || *char == '_' || *char == '-')
                .collect();
            if is_shortcode(&id) {
                return Some(id);
            }
        }
    }
    None
}

fn snowflake_in(url: &str) -> Option<String> {
    let rest = url.split("/status/").nth(1)?;
    let id: String = rest
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .collect();
    is_snowflake(&id).then_some(id)
}

fn scrub_seen(seen: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for id in seen {
        if is_post_id(&id) && !out.iter().any(|have| have == &id) {
            out.push(id);
        }
    }
    if out.len() > MAX_SEEN {
        let extra = out.len() - MAX_SEEN;
        out.drain(0..extra);
    }
    out
}

fn push_seen(seen: &mut Vec<String>, id: &str) {
    if !is_post_id(id) || seen.iter().any(|have| have == id) {
        return;
    }
    seen.push(id.to_string());
    if seen.len() > MAX_SEEN {
        let extra = seen.len() - MAX_SEEN;
        seen.drain(0..extra);
    }
}

fn finish_api(
    mut profile: Profile,
    platform: &str,
    index: usize,
    handle: &str,
    incoming: &Incoming,
    instagram: bool,
) -> Result<(Profile, bool), String> {
    let started = std::time::Instant::now();
    let (posts, display_name, avatar_url) = if instagram {
        let feed = super::ig::parse_profile(&incoming.raw, handle);
        (feed.posts, feed.display_name, feed.avatar)
    } else {
        let feed = super::weibo::parse_index(&incoming.raw, handle);
        (feed.posts, feed.display_name, feed.avatar)
    };
    let parse_ms = started.elapsed().as_secs_f64() * 1000.0;
    if !display_name.is_empty() {
        board_mut(&mut profile, platform).cards[index].label = text::chars_at_most(display_name.trim(), 80);
    }
    let avatar = clean_avatar(&avatar_url);
    if !avatar.is_empty() {
        board_mut(&mut profile, platform).cards[index].avatar = avatar;
    }
    let images_ms = 0.0;
    let total = incoming.load_ms as f64 + parse_ms;
    if instagram {
        eprintln!(
            "[pulse] instagram timing handle={handle} load={} parse={parse_ms:.0} images={images_ms:.0} total={total:.0}",
            incoming.load_ms
        );
    }
    if posts.is_empty() {
        return Ok((profile, false));
    }
    let keywords = profile.settings.mute_keywords.clone();
    let known_ids: Vec<String> = board(&profile, platform).cards[index]
        .posts
        .iter()
        .map(|post| post.id.clone())
        .collect();
    remember_fetch(platform, index, handle, &posts);
    let merged = merge_posts(
        take_posts(&mut board_mut(&mut profile, platform).cards[index].posts),
        posts,
        &keywords,
        platform,
    );
    let settings = profile.settings.clone();
    let card = &mut board_mut(&mut profile, platform).cards[index];
    card.posts = Arc::new(merged);
    card.last_fetch_ms = unix_ms();
    card.note.clear();
    if let Some(notice) = super::notice::consider_known(card, platform, &settings, &known_ids) {
        crate::notify::deliver(&notice);
    }
    Ok((profile, true))
}

fn login_note(platform: &str) -> String {
    match source::canon_platform(platform) {
        "instagram" => "需要登录 Instagram。可以在侧栏登录，Threads 也能用这个账号。".into(),
        "weibo" => "需要登录微博。可以在侧栏登录。".into(),
        "threads" => "需要登录 Threads。可以用 Instagram 账号登录。".into(),
        _ => "需要先在侧栏登录 X。登录状态会留在这台设备上。".into(),
    }
}

fn blocked_note(platform: &str) -> String {
    match source::canon_platform(platform) {
        "instagram" => "Instagram 这次没有给出帖子，稍后再试。".into(),
        "weibo" => "微博这次没有给出帖子，稍后再试。".into(),
        "threads" => "Threads 这次没有给出帖子，稍后再试。".into(),
        _ => "X 这次没有给出时间线，稍后再试。".into(),
    }
}

pub(crate) fn post_hidden(post: &Post, keywords: &[String]) -> bool {
    muted(post, keywords)
}

fn muted(post: &Post, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    let quote = post
        .quote
        .as_ref()
        .map(|quote| quote.text.as_str())
        .unwrap_or("");
    let hay = text::fold_space(&format!("{} {} {}", post.text, quote, post.link_title));
    keywords.iter().any(|keyword| {
        let needle = text::fold_space(keyword);
        !needle.is_empty() && hay.contains(&needle)
    })
}

pub fn sample_posts() -> Vec<PresentPost> {
    let posts = [
        Post {
            id: "sample-1".into(),
            text: "示例：这不是真实时间线。$NVDA 会标成彩色徽章，点开气泡会打开原帖。".into(),
            time_ms: 1_772_445_000_000,
            url: String::new(),
            images: Vec::new(),
            link_title: "示例：一篇虚构的笔记".into(),
            link_url: "https://example.com/sample-note".into(),
            quote: Some(Quote {
                author: "samplequote".into(),
                text: "引用也会收成一张小卡片，同样只是示例。".into(),
                url: String::new(),
            }),
        },
        Post {
            id: "sample-2".into(),
            text: "示例：$BTC 和普通文字排在一起。这里没有任何真实账号。".into(),
            time_ms: 1_772_358_600_000,
            url: String::new(),
            images: Vec::new(),
            link_title: String::new(),
            link_url: String::new(),
            quote: None,
        },
    ];
    let card = Card {
        seen: vec!["sample-1".into(), "sample-2".into()],
        ..Card::default()
    };
    posts.iter().map(|post| present_post(post, &card, false)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post(id: &str, time_ms: i64, text: &str) -> Post {
        Post {
            id: id.into(),
            text: text.into(),
            time_ms,
            url: format!("https://x.com/desk_sample/status/{id}"),
            ..Post::default()
        }
    }

    fn profile_with(handle: &str, posts: Vec<Post>) -> Profile {
        let mut profile = Profile::default();
        profile.x.cards.push(Card {
            handle: handle.into(),
            posts: Arc::new(posts),
            ..Card::default()
        });
        scrub_profile(profile)
    }

    #[test]
    fn a_profile_clone_does_not_copy_article_bodies() {
        let mut profile = Profile::default();
        let mut card = Card::default();
        card.handle = "https://example.com/feed.xml".into();
        Arc::make_mut(&mut card.posts).push(Post {
            id: "r0000000000000001".into(),
            text: "文".repeat(400_000),
            url: "https://example.com/1".into(),
            link_title: "标题".into(),
            link_url: "https://example.com/1".into(),
            time_ms: 1,
            ..Post::default()
        });
        profile.rss.cards.push(card);
        let started = std::time::Instant::now();
        let cloned = profile.clone();
        assert!(started.elapsed().as_millis() < 30);
        assert!(Arc::ptr_eq(
            &profile.rss.cards[0].posts,
            &cloned.rss.cards[0].posts
        ));
    }

    #[test]
    fn a_large_rss_card_keeps_a_short_tail() {
        let mut profile = Profile::default();
        let mut card = Card::default();
        card.handle = "https://example.com/feed.xml".into();
        let long = "文".repeat(5_000);
        for index in 1..=40 {
            Arc::make_mut(&mut card.posts).push(Post {
                id: format!("r{index:016x}"),
                text: long.clone(),
                time_ms: index,
                url: format!("https://example.com/{index}"),
                link_title: "标题".into(),
                link_url: format!("https://example.com/{index}"),
                ..Post::default()
            });
        }
        profile.rss.cards.push(card);
        let profile = scrub_profile(profile);
        let posts = &profile.rss.cards[0].posts;
        assert_eq!(posts.len(), MAX_RSS_POSTS);
        assert!(posts.iter().all(|post| post.text.chars().count() <= RSS_TEXT_CHARS));
        assert_eq!(posts.last().unwrap().time_ms, 40);
    }

    #[test]
    fn empty_rss_feeds_clear_in_one_step_and_a_live_feed_stays() {
        let mut profile = Profile::default();
        let mut live = Card::default();
        live.handle = "https://example.com/live.xml".into();
        live.label = "Live".into();
        live.last_fetch_ms = 10;
        Arc::make_mut(&mut live.posts).push(Post {
            id: "r0000000000000001".into(),
            text: "hello".into(),
            time_ms: 1,
            url: "https://example.com/1".into(),
            link_title: "t".into(),
            link_url: "https://example.com/1".into(),
            ..Post::default()
        });
        let mut empty = Card::default();
        empty.handle = "https://tech.trello.com/atom.xml".into();
        empty.label = "Trello".into();
        empty.note = "订阅没有内容。".into();
        profile.rss.cards.push(live);
        profile.rss.cards.push(empty);
        let catalog = rss_catalog(&profile);
        assert_eq!(catalog.feeds.len(), 2);
        assert!(!catalog.feeds[0].empty);
        assert!(catalog.feeds[1].empty);
        assert_eq!(catalog.feeds[1].host, "tech.trello.com");
        assert_eq!(clear_empty_rss(&mut profile), 1);
        assert_eq!(profile.rss.cards.len(), 1);
        assert_eq!(profile.rss.cards[0].label, "Live");
        assert_eq!(clear_empty_rss(&mut profile), 0);
    }

    #[test]
    fn chosen_rss_urls_are_removed_together() {
        let mut profile = Profile::default();
        for (label, url) in [("A", "https://a.example/feed"), ("B", "https://b.example/feed"), ("C", "https://c.example/feed")] {
            profile.rss.cards.push(Card {
                handle: url.into(),
                label: label.into(),
                ..Card::default()
            });
        }
        let removed = remove_rss_urls(
            &mut profile,
            &[
                "https://b.example/feed".into(),
                "https://missing.example/feed".into(),
                "https://a.example/feed".into(),
            ],
        );
        assert_eq!(removed, 2);
        assert_eq!(profile.rss.cards.len(), 1);
        assert_eq!(profile.rss.cards[0].label, "C");
    }

    #[test]
    fn rss_folders_group_feeds_and_survive_opml() {
        let mut profile = Profile::default();
        let news = create_rss_folder(&mut profile, "  新闻  ").unwrap();
        assert!(create_rss_folder(&mut profile, "新闻").is_err());
        profile.rss.cards.push(Card {
            handle: "https://news.example/rss".into(),
            label: "HN".into(),
            folder: news.clone(),
            ..Card::default()
        });
        profile.rss.cards.push(Card {
            handle: "https://loose.example/feed".into(),
            label: "散的".into(),
            ..Card::default()
        });
        let kept = timeline(&profile, "rss", 0, 10, &news);
        assert_eq!(kept.items.len(), 0);
        Arc::make_mut(&mut profile.rss.cards[0].posts).push(Post {
            id: "r000000000000000a".into(),
            text: "one".into(),
            time_ms: 5,
            ..Post::default()
        });
        Arc::make_mut(&mut profile.rss.cards[1].posts).push(Post {
            id: "r000000000000000b".into(),
            text: "two".into(),
            time_ms: 9,
            ..Post::default()
        });
        let only = timeline(&profile, "rss", 0, 10, &news);
        assert_eq!(only.items.len(), 1);
        assert_eq!(only.items[0].handle, "https://news.example/rss");
        let loose = timeline(&profile, "rss", 0, 10, "-");
        assert_eq!(loose.items[0].handle, "https://loose.example/feed");
        profile.x.cards.push(Card {
            handle: "desk".into(),
            label: "Desk".into(),
            folder: news.clone(),
            posts: Arc::new(vec![Post {
                id: "100".into(),
                text: "from x".into(),
                time_ms: 8,
                ..Post::default()
            }]),
            ..Card::default()
        });
        let mixed = timeline(&profile, "all", 0, 10, &news);
        assert_eq!(mixed.items.len(), 2);
        assert!(mixed.items.iter().any(|item| item.platform == "x"));
        assert!(mixed.items.iter().any(|item| item.platform == "rss"));
        let xml = export_opml(&profile);
        assert!(xml.contains("text=\"新闻\""));
        assert!(xml.contains("xmlUrl=\"https://loose.example/feed\""));
        let mut again = Profile::default();
        let changed = import_opml(&mut again, &xml).unwrap();
        assert_eq!(changed, 2);
        assert_eq!(again.folders.len(), 1);
        assert!(again.rss.folders.is_empty());
        assert_eq!(again.folders[0].name, "新闻");
        let hn = again.rss.cards.iter().find(|card| card.label == "HN").unwrap();
        assert_eq!(hn.folder, again.folders[0].id);
        assert!(again.rss.cards.iter().find(|card| card.label == "散的").unwrap().folder.is_empty());
        let renamed = again.folders[0].id.clone();
        rename_rss_folder(&mut again, &renamed, "时事").unwrap();
        assert_eq!(again.folders[0].name, "时事");
        assert_eq!(again.rss.cards.iter().find(|card| card.label == "HN").unwrap().folder, again.folders[0].id);
        let id = again.folders[0].id.clone();
        delete_rss_folder(&mut again, &id).unwrap();
        assert!(again.folders.is_empty());
        assert!(again.rss.cards.iter().all(|card| card.folder.is_empty()));
        let made = create_rss_folder(&mut again, "博客").unwrap();
        move_rss_urls(&mut again, &["https://loose.example/feed".into()], &made).unwrap();
        assert_eq!(
            again.rss.cards.iter().find(|card| card.label == "散的").unwrap().folder,
            made
        );
        let old = r#"{"version":1,"focus":"all","x":{"cards":[{"handle":"desk","label":"Desk","folder":"f1"}]},"threads":{"cards":[]},"instagram":{"cards":[]},"rss":{"folders":[{"id":"f1","name":"旧夹"}],"cards":[{"handle":"https://old.example/feed","label":"旧","folder":"f1"}]},"settings":{"refreshSecs":120,"muteKeywords":[],"theme":"system"}}"#;
        let loaded = normalize_json(old).unwrap();
        assert!(loaded.rss.folders.is_empty());
        assert_eq!(loaded.folders.len(), 1);
        assert_eq!(loaded.folders[0].name, "旧夹");
        assert_eq!(loaded.rss.cards[0].folder, "f1");
        assert_eq!(loaded.x.cards[0].folder, "f1");
        move_card_folder(&mut profile, "x", "desk", "").unwrap();
        assert!(profile.x.cards[0].folder.is_empty());
    }

    #[test]
    fn merges_in_chronological_order_and_counts_unread() {
        let profile = profile_with("desk_sample", vec![post("2", 2_000, "later $NVDA")]);
        let message = serde_json::json!({
            "type": "posts",
            "state": "ok",
            "handle": "Desk_Sample",
            "displayName": "Desk Sample",
            "posts": [post("1", 1_000, "earlier"), post("3", 3_000, "newest")]
        });
        let (profile, applied) = ingest(profile, "x", 0, &message.to_string()).unwrap();
        assert!(applied);
        assert_eq!(
            profile.x.cards[0]
                .posts
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["1", "2", "3"]
        );
        assert_eq!(profile.x.cards[0].label, "Desk Sample");
        let view = present_card(&profile, "x", 0).unwrap();
        assert_eq!(view.unread, 3);
        assert!(
            view.posts[1]
                .segments
                .iter()
                .any(|part| part.symbol == "NVDA")
        );
        let profile = mark_read(profile, "x", 0).unwrap();
        assert_eq!(present_card(&profile, "x", 0).unwrap().unread, 0);
        let again =
            serde_json::json!({"type":"posts","state":"ok","posts":[post("4", 4_000, "fresh")]})
                .to_string();
        let (profile, _) = ingest(profile, "x", 0, &again).unwrap();
        assert_eq!(present_card(&profile, "x", 0).unwrap().unread, 1);
        assert_eq!(unread_totals(&profile).x, 1);
    }

    #[test]
    fn a_newer_x_status_is_merged_and_only_a_real_parse_stamps_the_fetch() {
        let mut profile = profile_with("fxtrader", vec![post("2117000000000000001", 1_700_000_000_000, "morning")]);
        profile.x.cards[0].notify_ms = 1_700_000_000_000;
        profile.x.cards[0].last_fetch_ms = 50;
        profile.x.cards[0].seen.push("2117000000000000001".into());
        let empty = serde_json::json!({
            "type": "posts",
            "state": "empty",
            "handle": "fxtrader",
            "posts": []
        })
        .to_string();
        let (profile, applied) = ingest(profile, "x", 0, &empty).unwrap();
        assert!(applied);
        assert_eq!(profile.x.cards[0].last_fetch_ms, 50);
        assert_eq!(profile.x.cards[0].posts.len(), 1);
        assert_eq!(profile.x.cards[0].notify_ms, 1_700_000_000_000);

        let live = serde_json::json!({
            "type": "posts",
            "state": "ok",
            "handle": "fxtrader",
            "posts": [post("2117516318018843080", 1_700_000_100_000, "afternoon")]
        })
        .to_string();
        let (profile, applied) = ingest(profile, "x", 0, &live).unwrap();
        assert!(applied);
        assert_eq!(
            profile.x.cards[0]
                .posts
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["2117000000000000001", "2117516318018843080"]
        );
        assert_eq!(profile.x.cards[0].notify_ms, 1_700_000_100_000);
        assert!(profile.x.cards[0].last_fetch_ms > 1_700_000_000_000);
        assert_eq!(present_card(&profile, "x", 0).unwrap().unread, 1);
    }

    #[test]
    fn an_empty_scrape_does_not_erase_the_posts_already_parsed() {
        remember_fetch("threads", 4, "zixin_1010", &[]);
        remember_fetch(
            "threads",
            4,
            "zixin_1010",
            &[post("DReal123", 1_700_000_000_000, "caption")],
        );
        remember_fetch("threads", 4, "zixin_1010", &[]);
        let slots = fetch_slot().lock().unwrap();
        let note = slots.get(&("threads".into(), 4)).unwrap();
        assert_eq!(note.newest_id, "DReal123");
        assert_eq!(note.time_ms, 1_700_000_000_000);
        assert_eq!(note.posts, 1);
        assert_eq!(
            format_fetch_line("threads", "zixin_1010", &note.newest_id, note.time_ms, note.posts),
            "[pulse] threads fetch handle=zixin_1010 newest=DReal123 time=1700000000000 posts=1"
        );
        drop(slots);
        let _ = fetch_slot().lock().unwrap().remove(&("threads".into(), 4));
        assert_eq!(
            format_fetch_line("instagram", "aartisehwag", "", 0, 0),
            "[pulse] instagram fetch handle=aartisehwag newest=none time=0 posts=0"
        );
    }

    #[test]
    fn unknown_times_keep_page_order_and_an_empty_fetch_backs_off() {
        let mut profile = Profile::default();
        profile.instagram.cards.push(Card::default());
        profile.instagram.cards.push(Card::default());
        profile.instagram.cards.push(Card {
            handle: "aartisehwag".into(),
            ..Card::default()
        });
        let message = serde_json::json!({
            "type": "posts",
            "handle": "aartisehwag",
            "posts": [
                {"id": "GridBbbb", "text": "second on the page", "url": "https://www.instagram.com/p/GridBbbb/"},
                {"id": "GridAaaa", "text": "first on the page", "url": "https://www.instagram.com/p/GridAaaa/"}
            ]
        })
        .to_string();
        let (profile, applied) = ingest(profile, "instagram", 2, &message).unwrap();
        assert!(applied);
        assert_eq!(
            profile.instagram.cards[2]
                .posts
                .iter()
                .map(|post| post.id.as_str())
                .collect::<Vec<_>>(),
            ["GridBbbb", "GridAaaa"]
        );
        assert!(profile.instagram.cards[2].posts.iter().all(|post| post.time_ms == 0));
        let mut profile = profile;
        finish_fetch(&mut profile, "instagram", 2);
        assert_eq!(profile.instagram.cards[2].fail_streak, 0);
        assert!(profile.instagram.cards[2].last_attempt_ms > 0);

        let mut empty = Profile::default();
        empty.x.cards.push(Card::default());
        empty.x.cards.push(Card {
            handle: "huggingpapers".into(),
            ..Card::default()
        });
        finish_fetch(&mut empty, "x", 1);
        assert_eq!(empty.x.cards[1].fail_streak, 1);
        assert_eq!(empty.x.cards[1].last_fetch_ms, 0);
        assert!(empty.x.cards[1].last_attempt_ms > 0);
    }

    #[test]
    fn mute_drops_phrases_and_does_not_count_them() {
        let mut profile = profile_with("desk_sample", vec![post("1", 1_000, "hello")]);
        let message = serde_json::json!({
            "type": "posts",
            "posts": [
                post("2", 2_000, "A limited   time offer"),
                post("3", 3_000, "ordinary note")
            ]
        })
        .to_string();
        profile = set_mute(profile, "limited time\n赠送");
        let (profile, _) = ingest(profile, "x", 0, &message).unwrap();
        let ids: Vec<_> = profile.x.cards[0]
            .posts
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(ids, ["1", "3"]);
        assert_eq!(present_card(&profile, "x", 0).unwrap().unread, 2);
    }

    #[test]
    fn switching_handles_restores_the_cached_feed() {
        let mut profile = profile_with("alphaone", vec![post("9", 9_000, "from alpha")]);
        profile = set_handle(profile, "x", 0, "@betatwo").unwrap();
        assert!(profile.x.cards[0].posts.is_empty());
        assert_eq!(profile.x.cards[0].handle, "betatwo");
        let message =
            serde_json::json!({"type":"posts","posts":[post("8", 8_000, "from beta")]}).to_string();
        let (profile, _) = ingest(profile, "x", 0, &message).unwrap();
        let profile = set_handle(profile, "x", 0, "alphaone").unwrap();
        assert_eq!(profile.x.cards[0].posts[0].text, "from alpha");
        let profile = set_handle(profile, "x", 0, "@betatwo").unwrap();
        assert_eq!(profile.x.cards[0].posts[0].text, "from beta");
    }

    #[test]
    fn stale_page_and_bad_images_do_not_land() {
        let profile = profile_with("desk_sample", vec![]);
        let stale = serde_json::json!({"type":"posts","handle":"someoneelse","posts":[post("1", 1_000, "nope")]}).to_string();
        let (profile, applied) = ingest(profile, "x", 0, &stale).unwrap();
        assert!(!applied);
        assert!(profile.x.cards[0].posts.is_empty());
        let noisy = serde_json::json!({
            "type": "posts",
            "posts": [{
                "id": "15",
                "text": "chart https://example.com/story",
                "time": "2026-03-01T12:30:00Z",
                "url": "https://x.com/desk_sample/status/15",
                "images": ["https://evil.example/a.png", "https://pbs.twimg.com/media/abc.jpg?name=large"],
                "quote": {"author": "@SampleQuote", "text": "quoted $BTC", "url": "https://x.com/samplequote/status/4"}
            }]
        })
        .to_string();
        let (profile, applied) = ingest(profile, "x", 0, &noisy).unwrap();
        assert!(applied);
        let post = &profile.x.cards[0].posts[0];
        assert_eq!(
            post.images,
            vec!["https://pbs.twimg.com/media/abc.jpg?name=small"]
        );
        assert_eq!(post.link_url, "https://example.com/story");
        assert_eq!(post.link_title, "example.com");
        assert_eq!(post.quote.as_ref().unwrap().author, "samplequote");
        let long = format!(
            "https://scontent.cdninstagram.com/v/{}",
            "a".repeat(1_200)
        );
        let kept = super::thumb(&long).unwrap();
        assert!(kept.len() > 800);
        let video = super::thumb("https://pbs.twimg.com/amplify_video_thumb/123/img/abc.jpg").unwrap();
        assert!(video.contains("name=small"));
        assert_eq!(
            post.time_ms,
            super::super::time::parse_time_ms("2026-03-01T12:30:00Z").unwrap()
        );
    }

    #[test]
    fn import_clamps_and_rejects_the_wrong_file() {
        let mut profile = Profile::default();
        profile.settings.refresh_secs = 1;
        profile.settings.theme = "blue".into();
        profile.x.cards.push(Card {
            handle: "Desk_Sample".into(),
            ..Card::default()
        });
        let json = serde_json::to_string(&profile).unwrap();
        let loaded = normalize_json(&json).unwrap();
        assert_eq!(loaded.settings.refresh_secs, MIN_REFRESH_SECS);
        assert_eq!(loaded.settings.theme, "system");
        assert_eq!(loaded.x.cards.len(), 1);
        assert_eq!(loaded.x.cards[0].handle, "desk_sample");
        assert!(normalize_json("[]").is_err());
        assert!(normalize_json(r#"{"version":2}"#).is_err());
        let again = normalize_json(&serde_json::to_string(&loaded).unwrap()).unwrap();
        assert_eq!(again, loaded);
    }

    #[test]
    fn legacy_cards_move_onto_their_own_lists() {
        let json = r#"{
            "version": 1,
            "layout": 1,
            "cards": [
                { "platform": "instagram", "handle": "Sample.Name", "label": "IG", "posts": [], "seen": [], "lastFetchMs": 4, "note": "" },
                { "handle": "Desk_Sample", "label": "Desk", "posts": [], "seen": [], "lastFetchMs": 0, "note": "" },
                { "platform": "threads", "handle": "threaduser", "label": "", "posts": [], "seen": [], "lastFetchMs": 0, "note": "" },
                { "platform": "x", "handle": "secondx", "label": "", "posts": [], "seen": [], "lastFetchMs": 0, "note": "" }
            ],
            "settings": { "refreshSecs": 120, "muteKeywords": [], "theme": "system" }
        }"#;
        let loaded = normalize_json(json).unwrap();
        assert_eq!(loaded.focus, "all");
        assert_eq!(loaded.settings.home, "timeline");
        assert_eq!(loaded.x.cards.len(), 2);
        assert_eq!(loaded.x.cards[0].handle, "desk_sample");
        assert_eq!(loaded.x.cards[1].handle, "secondx");
        assert_eq!(loaded.threads.cards[0].handle, "threaduser");
        assert_eq!(loaded.instagram.cards[0].handle, "sample.name");
        assert_eq!(loaded.instagram.cards[0].label, "IG");
        assert_eq!(loaded.instagram.cards[0].last_fetch_ms, 4);
        assert_eq!(
            present_card(&loaded, "x", 0).unwrap().url,
            "https://x.com/desk_sample"
        );
        assert_eq!(
            present_card(&loaded, "threads", 0).unwrap().url,
            "https://www.threads.net/@threaduser"
        );
        let wrong = set_handle(loaded.clone(), "x", 2, "https://www.threads.net/@someone");
        assert!(wrong.unwrap_err().contains("另一个平台"));
        let profile = set_handle(loaded, "threads", 1, "@otherthread").unwrap();
        assert_eq!(profile.threads.cards[1].handle, "otherthread");
        assert_eq!(profile.x.cards[0].handle, "desk_sample");
        let saved = serde_json::to_string(&profile).unwrap();
        assert!(saved.contains("threaduser"));
        assert!(saved.contains("sample.name"));
        assert!(saved.contains("desk_sample"));
        let again = normalize_json(&saved).unwrap();
        assert_eq!(again, profile);
        let mixed = set_focus(again, "all").unwrap();
        let keys: Vec<_> = focus_slots(&mixed)
            .iter()
            .map(|slot| format!("{}:{}", slot.platform, slot.index))
            .collect();
        assert_eq!(
            keys,
            vec!["x:0", "x:1", "threads:0", "threads:1", "instagram:0"]
        );
    }

    #[test]
    fn instagram_and_threads_posts_keep_shortcodes() {
        let mut profile = Profile::default();
        profile.instagram.cards.push(Card {
            handle: "sample.name".into(),
            ..Card::default()
        });
        let message = serde_json::json!({
            "type": "posts",
            "state": "ok",
            "posts": [{
                "id": "AbCdEf1",
                "text": "a reel $BTC",
                "timeMs": 1_700_000_000_000_i64,
                "url": "https://www.instagram.com/reel/AbCdEf1/",
                "images": ["https://scontent.cdninstagram.com/v/photo.jpg"]
            }]
        })
        .to_string();
        let (profile, applied) = ingest(profile, "instagram", 0, &message).unwrap();
        assert!(applied);
        assert_eq!(profile.instagram.cards[0].posts[0].id, "AbCdEf1");
        assert!(profile.instagram.cards[0].posts[0].images[0].contains("cdninstagram"));
        let signed_out = serde_json::json!({"type":"session","state":"signed-out"}).to_string();
        let (mut profile, _) = ingest(profile, "instagram", 0, &signed_out).unwrap();
        assert!(profile.instagram.cards[0].note.contains("Instagram"));
        profile.threads.cards.push(Card {
            handle: "kept".into(),
            ..Card::default()
        });
        profile.threads.cards.push(Card {
            handle: "also".into(),
            ..Card::default()
        });
        profile.threads.cards.push(Card {
            handle: "sampledesk".into(),
            ..Card::default()
        });
        let threads = serde_json::json!({
            "type": "posts",
            "posts": [{
                "id": "CxYz12ab",
                "text": "hello",
                "time": "2026-03-01T12:30:00Z",
                "url": "https://www.threads.net/@sampledesk/post/CxYz12ab"
            }]
        })
        .to_string();
        let (profile, _) = ingest(profile, "threads", 2, &threads).unwrap();
        assert_eq!(profile.threads.cards[2].posts[0].id, "CxYz12ab");
        assert!(profile.threads.cards[2].posts[0].url.contains("/post/CxYz12ab"));
        assert!(profile.x.cards.is_empty());
    }

    #[test]
    fn refresh_makes_visible_cards_due_again() {
        let mut profile = profile_with("desk_sample", vec![]);
        profile.focus = "x".into();
        profile.x.cards[0].last_fetch_ms = 50_000;
        profile.x.cards.push(Card {
            handle: "other".into(),
            last_fetch_ms: 50_000,
            ..Card::default()
        });
        profile.threads.cards.push(Card {
            handle: "threaduser".into(),
            last_fetch_ms: 50_000,
            ..Card::default()
        });
        let profile = request_refresh(profile);
        assert_eq!(profile.x.cards[0].last_fetch_ms, 0);
        assert_eq!(profile.x.cards[1].last_fetch_ms, 0);
        assert_eq!(profile.threads.cards[0].last_fetch_ms, 50_000);
    }

    #[test]
    fn the_mixed_timeline_is_newest_first_and_a_chip_filters_it() {
        let mut profile = Profile::default();
        profile.focus = "all".into();
        profile.settings.mute_keywords = vec!["静音".into()];
        profile.x.cards.push(Card {
            handle: "desk".into(),
            label: "Desk".into(),
            posts: Arc::new(vec![
                Post {
                    id: "100".into(),
                    text: "older x".into(),
                    time_ms: 1_000,
                    url: "https://x.com/desk/status/100".into(),
                    ..Post::default()
                },
                Post {
                    id: "200".into(),
                    text: "静音词".into(),
                    time_ms: 9_000,
                    url: "https://x.com/desk/status/200".into(),
                    ..Post::default()
                },
            ]),
            ..Card::default()
        });
        profile.rss.cards.push(Card {
            handle: "https://example.com/feed.xml".into(),
            label: "Example".into(),
            posts: Arc::new(vec![Post {
                id: "r000000000000000a".into(),
                text: "rss body".into(),
                time_ms: 5_000,
                url: "https://example.com/a".into(),
                link_title: "标题".into(),
                link_url: "https://example.com/a".into(),
                ..Post::default()
            }]),
            ..Card::default()
        });
        let page = timeline(&profile, "", 0, 10, "");
        assert_eq!(page.total, 2);
        assert_eq!(page.items[0].platform, "rss");
        assert_eq!(page.items[0].author, "Example");
        assert_eq!(page.items[0].avatar, "");
        assert_eq!(page.items[1].platform, "x");
        assert_eq!(page.items[1].author, "Desk");
        assert_eq!(page.items[1].avatar, "");
        let only = timeline(&profile, "x", 0, 10, "");
        assert_eq!(only.total, 1);
        assert_eq!(only.items[0].post.id, "100");
        assert!(mark_post_read(&mut profile, "x", 0, "100"));
        assert!(!mark_post_read(&mut profile, "x", 0, "100"));
        assert!(!timeline(&profile, "x", 0, 10, "").items[0].post.unread);
    }

    #[test]
    fn an_older_save_opens_on_the_mixed_timeline() {
        let raw = r#"{"version":1,"focus":"rss","x":{"cards":[]},"threads":{"cards":[]},"instagram":{"cards":[]},"weibo":{"cards":[]},"rss":{"cards":[]},"cache":[],"settings":{"refreshSecs":120,"muteKeywords":[],"theme":"system"}}"#;
        let profile = normalize_json(raw).unwrap();
        assert_eq!(profile.focus, "all");
        assert_eq!(profile.settings.home, "timeline");
        let again = normalize_json(&serde_json::to_string(&profile).unwrap()).unwrap();
        let kept = set_focus(again, "rss").unwrap();
        let saved = set_home(kept, "board").unwrap();
        let loaded = normalize_json(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(loaded.focus, "rss");
        assert_eq!(loaded.settings.home, "board");
    }

    #[test]
    fn choosing_a_platform_sticks_through_save_and_reload() {
        let mut profile = Profile::default();
        profile.threads.cards.push(Card {
            handle: "threaduser".into(),
            ..Card::default()
        });
        profile.instagram.cards.push(Card {
            handle: "sample.name".into(),
            ..Card::default()
        });
        profile = set_focus(profile, "threads").unwrap();
        assert_eq!(profile.focus, "threads");
        assert!(focus_slots(&profile).iter().all(|slot| slot.platform == "threads"));
        let loaded = normalize_json(&serde_json::to_string(&profile).unwrap()).unwrap();
        assert_eq!(loaded.focus, "threads");
        assert_eq!(loaded.threads.cards[0].handle, "threaduser");
        assert!(focus_slots(&loaded).iter().all(|slot| slot.platform == "threads"));
        profile = set_focus(loaded, "instagram").unwrap();
        let loaded = normalize_json(&serde_json::to_string(&profile).unwrap()).unwrap();
        assert_eq!(loaded.focus, "instagram");
        assert_eq!(focus_slots(&loaded)[0].platform, "instagram");
        profile = set_focus(loaded, "all").unwrap();
        let loaded = normalize_json(&serde_json::to_string(&profile).unwrap()).unwrap();
        assert_eq!(loaded.focus, "all");
        let slots = focus_slots(&loaded);
        let keys: Vec<_> = slots.iter().map(|slot| slot.platform.as_str()).collect();
        assert!(keys.contains(&"threads") && keys.contains(&"instagram"));
        assert!(set_focus(loaded, "discord").is_err());
    }

    #[test]
    fn a_grid_save_drops_empty_slots_and_new_accounts_append() {
        let json = r#"{
            "version": 1,
            "focus": "threads",
            "layout": 3,
            "allLayout": 1,
            "x": { "layout": 1, "cards": [
                { "handle": "one" },
                { "handle": "" },
                { "handle": "two" }
            ] }
        }"#;
        let loaded = normalize_json(json).unwrap();
        assert_eq!(loaded.focus, "all");
        assert_eq!(loaded.settings.home, "timeline");
        assert_eq!(
            loaded.x.cards.iter().map(|card| card.handle.as_str()).collect::<Vec<_>>(),
            ["one", "two"]
        );
        let profile = set_handle(loaded, "x", 2, "@three").unwrap();
        assert_eq!(profile.x.cards[2].handle, "three");
        let profile = move_card(profile, "x", 2, -1).unwrap();
        assert_eq!(
            profile.x.cards.iter().map(|card| card.handle.as_str()).collect::<Vec<_>>(),
            ["one", "three", "two"]
        );
        let profile = set_handle(profile, "x", 1, "").unwrap();
        assert_eq!(
            profile.x.cards.iter().map(|card| card.handle.as_str()).collect::<Vec<_>>(),
            ["one", "two"]
        );
        let again = normalize_json(&serde_json::to_string(&profile).unwrap()).unwrap();
        assert_eq!(again.focus, "all");
        assert_eq!(again.settings.home, "timeline");
        assert_eq!(again.x.cards.len(), 2);
    }

    #[test]
    fn a_relative_time_is_not_the_caption_and_signed_photo_urls_stay() {
        let mut profile = Profile::default();
        profile.threads.cards.push(Card {
            handle: "kyung31_".into(),
            ..Card::default()
        });
        let long = format!("https://scontent-nrt1-1.cdninstagram.com/v/{}", "a".repeat(5_000));
        let message = serde_json::json!({
            "type": "posts",
            "displayName": "Kyung",
            "avatar": "https://scontent.cdninstagram.com/v/avatar.jpg",
            "posts": [
                {
                    "id": "CxYz12ab",
                    "text": "2d",
                    "timeMs": 1_700_000_000_000_i64,
                    "url": "https://www.threads.net/@kyung31_/post/CxYz12ab"
                },
                {
                    "id": "AbCdEf2x",
                    "text": "6h",
                    "timeMs": 1_700_000_000_100_i64,
                    "url": "https://www.threads.net/@kyung31_/post/AbCdEf2x",
                    "images": [long]
                },
                {
                    "id": "Hello99a",
                    "text": "a real caption",
                    "timeMs": 1_700_000_000_200_i64,
                    "url": "https://www.threads.net/@kyung31_/post/Hello99a"
                }
            ]
        })
        .to_string();
        let (profile, applied) = ingest(profile, "threads", 0, &message).unwrap();
        assert!(applied);
        assert_eq!(profile.threads.cards[0].label, "Kyung");
        assert!(profile.threads.cards[0].avatar.contains("cdninstagram"));
        let ids: Vec<_> = profile.threads.cards[0]
            .posts
            .iter()
            .map(|post| post.id.as_str())
            .collect();
        assert!(!ids.contains(&"CxYz12ab"));
        assert!(ids.contains(&"AbCdEf2x"));
        assert!(ids.contains(&"Hello99a"));
        let photo = profile.threads.cards[0]
            .posts
            .iter()
            .find(|post| post.id == "AbCdEf2x")
            .unwrap();
        assert!(photo.text.is_empty());
        assert!(photo.images[0].len() > 4_000);
        assert!(photo.images[0].contains("scontent-nrt1-1.cdninstagram.com"));
        let caption = profile.threads.cards[0]
            .posts
            .iter()
            .find(|post| post.id == "Hello99a")
            .unwrap();
        assert_eq!(caption.text, "a real caption");
    }

    #[test]
    fn samples_are_obviously_fake() {
        let posts = sample_posts();
        let blob = serde_json::to_string(&posts).unwrap();
        assert!(blob.contains("示例"));
        assert!(blob.contains("$NVDA") || blob.contains("NVDA"));
        assert!(!blob.to_ascii_lowercase().contains("elon"));
        assert!(posts.iter().all(|post| post.url.is_empty()));
    }

    #[test]
    fn a_weibo_uid_is_saved_and_listed_with_everything() {
        let old = r#"{"version":1,"focus":"x","x":{"cards":[]},"threads":{"cards":[]},"instagram":{"cards":[]},"cache":[],"settings":{"refreshSecs":120}}"#;
        let mut profile = normalize_json(old).unwrap();
        assert!(profile.weibo.cards.is_empty());
        add_weibo(
            &mut profile,
            0,
            "1319518957",
            "投星资产",
            "https://tva.sinaimg.cn/a.jpg",
        )
        .unwrap();
        assert_eq!(profile.weibo.cards[0].handle, "1319518957");
        assert_eq!(profile.weibo.cards[0].label, "投星资产");
        assert_eq!(profile.weibo.cards[0].avatar, "https://tva.sinaimg.cn/a.jpg");
        let saved = serde_json::to_string(&profile).unwrap();
        assert!(saved.contains("\"weibo\""));
        let mut again = normalize_json(&saved).unwrap();
        assert_eq!(again.weibo.cards[0].handle, "1319518957");
        assert_eq!(again.weibo.cards[0].avatar, "https://tva.sinaimg.cn/a.jpg");
        again.focus = "all".into();
        assert!(focus_slots(&again).iter().any(|slot| slot.platform == "weibo" && slot.index == 0));
        let missed = super::super::weibo::find_user(r#"{"data":{"cards":[]}}"#, "投星资产");
        assert!(missed.is_none());
    }

    #[test]
    fn a_weibo_parse_without_userinfo_keeps_the_nickname_and_photo() {
        let mut profile = Profile::default();
        add_weibo(&mut profile, 0, "2014433131", "", "").unwrap();
        assert!(profile.weibo.cards[0].label.is_empty());
        let raw = r#"{"data":{"cards":[{"mblog":{"id":"4999887766","created_at":"Wed Nov 04 12:00:00 +0800 2026","text":"正文","user":{"screen_name":"台账","avatar_hd":"https://tvax1.sinaimg.cn/a.jpg"}}}]}}"#;
        let message = serde_json::json!({
            "type": "weibo-api",
            "state": "ok",
            "handle": "2014433131",
            "displayName": "",
            "avatar": "",
            "raw": raw
        });
        let (profile, applied) = ingest(profile, "weibo", 0, &message.to_string()).unwrap();
        assert!(applied);
        assert_eq!(profile.weibo.cards[0].label, "台账");
        assert_eq!(profile.weibo.cards[0].avatar, "https://tvax1.sinaimg.cn/a.jpg");
        let page = timeline(&profile, "weibo", 0, 10, "");
        assert_eq!(page.items[0].author, "台账");
        assert_eq!(page.items[0].avatar, "https://tvax1.sinaimg.cn/a.jpg");
        assert_ne!(page.items[0].author, "@2014433131");
    }
}
