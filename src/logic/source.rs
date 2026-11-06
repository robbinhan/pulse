//! What a card can follow: X, Threads, or Instagram.
//!
//! A missing platform in an older save means X. Discord is recognized so a
//! later build can attach it, but it does not load a page. Linux (GTK 4 +
//! WebKitGTK 6.0) and iOS reuse the same profile; the shell chooses a grid
//! or a column in `schedule::shell_mode`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceHit {
    pub kind: &'static str,
    pub supported: bool,
    pub handle: String,
    pub url: String,
    pub message: String,
}

const X_RESERVED: &[&str] = &[
    "home",
    "explore",
    "search",
    "settings",
    "i",
    "intent",
    "share",
    "login",
    "signup",
    "compose",
    "messages",
    "notifications",
    "tos",
    "privacy",
    "hashtag",
    "jobs",
    "about",
    "download",
];

const IG_RESERVED: &[&str] = &[
    "p",
    "reel",
    "reels",
    "stories",
    "explore",
    "accounts",
    "direct",
    "about",
    "legal",
    "emails",
    "developer",
    "login",
];

const THREADS_RESERVED: &[&str] = &[
    "login", "intent", "search", "activity", "explore", "post", "t",
];

/// A platform whose profile page can be loaded in a hidden web view.
pub trait FeedAdapter {
    fn id(&self) -> &'static str;
    fn supported(&self) -> bool;
    fn profile_url(&self, handle: &str) -> Option<String>;
}

pub struct XAdapter;
pub struct ThreadsAdapter;
pub struct InstagramAdapter;
pub struct DiscordAdapter;

impl FeedAdapter for XAdapter {
    fn id(&self) -> &'static str {
        "x"
    }
    fn supported(&self) -> bool {
        true
    }
    fn profile_url(&self, handle: &str) -> Option<String> {
        let hit = parse_source_for(handle, "x");
        (hit.supported && hit.kind == "x").then_some(hit.url)
    }
}

impl FeedAdapter for ThreadsAdapter {
    fn id(&self) -> &'static str {
        "threads"
    }
    fn supported(&self) -> bool {
        true
    }
    fn profile_url(&self, handle: &str) -> Option<String> {
        let hit = parse_source_for(handle, "threads");
        (hit.supported && hit.kind == "threads").then_some(hit.url)
    }
}

impl FeedAdapter for InstagramAdapter {
    fn id(&self) -> &'static str {
        "instagram"
    }
    fn supported(&self) -> bool {
        true
    }
    fn profile_url(&self, handle: &str) -> Option<String> {
        let hit = parse_source_for(handle, "instagram");
        (hit.supported && hit.kind == "instagram").then_some(hit.url)
    }
}

struct WeiboAdapter;

impl FeedAdapter for WeiboAdapter {
    fn id(&self) -> &'static str {
        "weibo"
    }
    fn supported(&self) -> bool {
        true
    }
    fn profile_url(&self, handle: &str) -> Option<String> {
        let hit = parse_source_for(handle, "weibo");
        (hit.supported && hit.kind == "weibo").then_some(hit.url)
    }
}

impl FeedAdapter for DiscordAdapter {
    fn id(&self) -> &'static str {
        "discord"
    }
    fn supported(&self) -> bool {
        false
    }
    fn profile_url(&self, _handle: &str) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePlan {
    pub id: &'static str,
    pub supported: bool,
    pub example: String,
}

/// X, Threads, Instagram, and Weibo load. Discord is recognized and not loaded.
pub fn planned_sources() -> Vec<SourcePlan> {
    [
        &XAdapter as &dyn FeedAdapter,
        &ThreadsAdapter,
        &InstagramAdapter,
        &WeiboAdapter,
        &DiscordAdapter,
    ]
    .into_iter()
    .map(|adapter| SourcePlan {
        id: adapter.id(),
        supported: adapter.supported(),
        example: adapter.profile_url("sampledesk").unwrap_or_default(),
    })
    .collect()
}

/// `x`, `threads`, or `instagram`. Anything else, including empty, is X.
pub fn canon_platform(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "threads" | "thread" => "threads",
        "instagram" | "ig" => "instagram",
        "weibo" | "wb" => "weibo",
        "rss" | "feed" => "rss",
        _ => "x",
    }
}

pub fn login_url(platform: &str) -> String {
    match canon_platform(platform) {
        "threads" => "https://www.threads.net/login".into(),
        "instagram" => "https://www.instagram.com/accounts/login/".into(),
        "weibo" => "https://m.weibo.cn/login".into(),
        "rss" => String::new(),
        _ => "https://x.com/login".into(),
    }
}

/// Desktop Weibo passport. `url` is where SSO should land so `.weibo.cn` gets SUB.
pub const WEIBO_DESKTOP_LOGIN: &str = "https://passport.weibo.com/sso/signin?entry=miniblog&source=miniblog&disp=popup&url=https%3A%2F%2Fm.weibo.cn%2F";

/// Phone Weibo passport. `m.weibo.cn/login` is a 404, so the sheet opens the wap SSO page.
pub const WEIBO_IOS_LOGIN: &str =
    "https://passport.weibo.com/sso/signin?entry=wapsso&source=wapssowb&url=https%3A%2F%2Fm.weibo.cn%2F";

/// Login page for this OS.
/// Desktop Weibo uses the passport QR page; iOS uses the wap SSO page.
/// iOS Threads must be `threads.com`, the host the feed and the probe use.
/// `threads.net` keeps a separate cookie jar, then the page jumps to an
/// Instagram accounts document that stays blank in the web view.
pub fn login_page(platform: &str, os: &str) -> String {
    if canon_platform(platform) == "weibo" {
        if os.eq_ignore_ascii_case("ios") {
            return WEIBO_IOS_LOGIN.into();
        }
        return WEIBO_DESKTOP_LOGIN.into();
    }
    if canon_platform(platform) == "threads" && os.eq_ignore_ascii_case("ios") {
        return "https://www.threads.com/login".into();
    }
    login_url(platform)
}

pub fn home_url(platform: &str) -> String {
    match canon_platform(platform) {
        "threads" => "https://www.threads.com/".into(),
        "instagram" => "https://www.instagram.com/".into(),
        "weibo" => "https://m.weibo.cn/".into(),
        "rss" => String::new(),
        _ => "https://x.com/home".into(),
    }
}

pub fn profile_url(platform: &str, handle: &str) -> Option<String> {
    if canon_platform(platform) == "rss" {
        return super::rss::canonical_feed_url(handle);
    }
    let hit = parse_source_for(handle, platform);
    hit.supported.then_some(hit.url)
}

/// Bare `@name` is read as X, the same as older builds.
pub fn parse_source(input: &str) -> SourceHit {
    parse_source_for(input, "x")
}

/// `preferred` is the platform picker. A URL or a `threads:` / `instagram:` prefix wins.
pub fn parse_source_for(input: &str, preferred: &str) -> SourceHit {
    let text = input.trim();
    if text.is_empty() {
        return invalid("先输入 @handle。");
    }
    if canon_platform(preferred) == "rss" {
        return finish_rss(text);
    }
    let lower = text.to_ascii_lowercase();
    if let Some(rest) = strip_prefix(&lower, &["threads:", "threads/", "threads@"]) {
        return finish_threads(rest.trim().trim_start_matches('@'));
    }
    if let Some(rest) = strip_prefix(&lower, &["instagram:", "ig:"]) {
        return finish_instagram(rest.trim().trim_start_matches('@'));
    }
    if let Some(rest) = strip_prefix(&lower, &["weibo:", "wb:"]) {
        return finish_weibo(rest.trim().trim_start_matches('@'));
    }
    if lower.contains("discord.com") || lower.contains("discord.gg") {
        return unsupported(
            "discord",
            "这个链接还不能加。",
        );
    }
    if text.starts_with("http://") || text.starts_with("https://") || looks_like_url(text) {
        if let Some(hit) = parse_url(text) {
            return hit;
        }
        return invalid("这个链接还认不出来。可以填 @name，或贴 X、Threads、Instagram、微博的主页。");
    }
    let handle = text.trim_start_matches('@');
    match canon_platform(preferred) {
        "threads" => finish_threads(handle),
        "instagram" => finish_instagram(handle),
        "weibo" => finish_weibo(handle),
        _ => finish_x(handle),
    }
}

pub fn is_handle(handle: &str) -> bool {
    is_x_handle(handle)
}

pub fn valid_handle(platform: &str, handle: &str) -> bool {
    match canon_platform(platform) {
        "threads" => {
            is_ig_handle(handle)
                && !THREADS_RESERVED.contains(&handle.to_ascii_lowercase().as_str())
        }
        "instagram" => {
            is_ig_handle(handle) && !IG_RESERVED.contains(&handle.to_ascii_lowercase().as_str())
        }
        "weibo" => is_weibo_uid(handle),
        "rss" => super::rss::canonical_feed_url(handle).is_some(),
        _ => is_x_handle(handle) && !X_RESERVED.contains(&handle.to_ascii_lowercase().as_str()),
    }
}

fn strip_prefix<'a>(text: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|prefix| text.strip_prefix(prefix))
}

fn looks_like_url(text: &str) -> bool {
    if text.contains(' ') {
        return false;
    }
    if text.contains('/') {
        return true;
    }
    let lower = text.to_ascii_lowercase();
    [
        ".com",
        ".net",
        "threads.",
        "instagram.",
        "twitter.",
        "discord.",
        "weibo.",
    ]
    .iter()
    .any(|part| lower.contains(part))
}

fn parse_url(input: &str) -> Option<SourceHit> {
    let with_scheme = if input.starts_with("http://") || input.starts_with("https://") {
        input.to_string()
    } else {
        format!("https://{input}")
    };
    let rest = with_scheme
        .strip_prefix("https://")
        .or_else(|| with_scheme.strip_prefix("http://"))?;
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    let host = host.split(':').next().unwrap_or(host);
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if host == "discord.com" || host == "discord.gg" || host.ends_with(".discord.com") {
        return Some(unsupported(
            "discord",
            "这个链接还不能加。",
        ));
    }
    if host == "threads.net"
        || host == "threads.com"
        || host.ends_with(".threads.net")
        || host.ends_with(".threads.com")
    {
        if segments.iter().any(|part| *part == "post" || *part == "t") {
            return Some(invalid("请填 Threads 账号，不要贴单条帖子。"));
        }
        let handle = segments
            .first()
            .copied()
            .unwrap_or("")
            .trim_start_matches('@');
        if handle.is_empty() {
            return Some(invalid("这是 Threads 的首页，不是一个账号。"));
        }
        return Some(finish_threads(handle));
    }
    if host == "instagram.com" || host.ends_with(".instagram.com") {
        if segments.is_empty() {
            return Some(invalid("这是 Instagram 的首页，不是一个账号。"));
        }
        if matches!(segments[0], "p" | "reel" | "reels" | "stories" | "accounts") {
            return Some(invalid("请填 Instagram 账号，不要贴单条帖子或登录页。"));
        }
        return Some(finish_instagram(segments[0].trim_start_matches('@')));
    }
    if host == "weibo.com" || host == "m.weibo.cn" || host.ends_with(".weibo.com") || host.ends_with(".weibo.cn") {
        return Some(weibo_from_path(&segments));
    }
    if matches!(
        host,
        "x.com" | "twitter.com" | "mobile.twitter.com" | "mobile.x.com"
    ) {
        if segments.is_empty() {
            return Some(invalid("这是 X 的首页，不是一个账号。"));
        }
        if segments.len() > 1 {
            return Some(invalid("请填账号，不要贴单条帖子的链接。"));
        }
        return Some(finish_x(segments[0].trim_start_matches('@')));
    }
    None
}

fn finish_x(handle: &str) -> SourceHit {
    if !is_x_handle(handle) {
        return invalid("X 账号只能是 1–15 位字母、数字或下划线。");
    }
    if X_RESERVED.contains(&handle.to_ascii_lowercase().as_str()) {
        return invalid("这是 X 的页面，不是一个账号。");
    }
    let handle = handle.to_ascii_lowercase();
    SourceHit {
        kind: "x",
        supported: true,
        url: format!("https://x.com/{handle}"),
        handle,
        message: String::new(),
    }
}

fn finish_threads(handle: &str) -> SourceHit {
    if !is_ig_handle(handle) {
        return invalid("Threads 账号只能是 1–30 位字母、数字、下划线或点。");
    }
    if THREADS_RESERVED.contains(&handle.to_ascii_lowercase().as_str()) {
        return invalid("这是 Threads 的页面，不是一个账号。");
    }
    let handle = handle.to_ascii_lowercase();
    SourceHit {
        kind: "threads",
        supported: true,
        url: format!("https://www.threads.net/@{handle}"),
        handle,
        message: String::new(),
    }
}

fn finish_rss(raw: &str) -> SourceHit {
    match super::rss::canonical_feed_url(raw) {
        Some(url) => SourceHit {
            kind: "rss",
            supported: true,
            handle: url.clone(),
            url,
            message: String::new(),
        },
        None => invalid("请贴订阅地址，以 https:// 开头。"),
    }
}

fn finish_weibo(uid: &str) -> SourceHit {
    let uid = uid.trim().trim_start_matches('@');
    if !is_weibo_uid(uid) {
        return invalid("微博账号填数字 uid，或贴 weibo.com/u/uid 、m.weibo.cn/u/uid。");
    }
    SourceHit {
        kind: "weibo",
        supported: true,
        url: format!("https://m.weibo.cn/u/{uid}"),
        handle: uid.to_string(),
        message: String::new(),
    }
}

/// What the add-account field means on the Weibo tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WeiboInput {
    /// A numeric uid, from the field itself or from `/u/` or `/profile/`.
    Uid { uid: String, query: String },
    /// A nickname still to look up with the logged-in mobile session.
    Name { query: String },
    Reject { message: String },
}

pub fn read_weibo_input(input: &str) -> WeiboInput {
    let text = input.trim();
    if text.is_empty() {
        return WeiboInput::Reject {
            message: "先填写微博昵称、uid 或主页链接。".into(),
        };
    }
    if text.starts_with("http://") || text.starts_with("https://") || looks_like_url(text) {
        return match parse_url(text) {
            Some(hit) if hit.kind == "weibo" && hit.supported && is_weibo_uid(&hit.handle) => {
                WeiboInput::Uid {
                    uid: hit.handle,
                    query: text.to_string(),
                }
            }
            Some(hit) if hit.kind == "weibo-name" => WeiboInput::Name { query: hit.handle },
            Some(hit) if !hit.message.is_empty() => WeiboInput::Reject { message: hit.message },
            _ => WeiboInput::Reject {
                message: "没找到这个微博账号".into(),
            },
        };
    }
    let handle = text.trim_start_matches('@');
    if is_weibo_uid(handle) {
        WeiboInput::Uid {
            uid: handle.to_string(),
            query: text.to_string(),
        }
    } else if weibo_nickname(handle) {
        WeiboInput::Name {
            query: handle.to_string(),
        }
    } else {
        WeiboInput::Reject {
            message: "没找到这个微博账号".into(),
        }
    }
}

/// Uid in a Weibo profile URL (`/u/<uid>` or `/profile/<uid>`), else empty.
pub fn weibo_uid_in_url(url: &str) -> String {
    match read_weibo_input(url) {
        WeiboInput::Uid { uid, .. } => uid,
        _ => String::new(),
    }
}

fn weibo_from_path(segments: &[&str]) -> SourceHit {
    if segments.is_empty() {
        return invalid("这是微博的首页，不是一个账号。");
    }
    if segments[0] == "u" || segments[0] == "profile" {
        return finish_weibo(segments.get(1).copied().unwrap_or(""));
    }
    if segments[0] == "n" {
        let name = percent_decode(segments.get(1).copied().unwrap_or(""));
        return weibo_name_hit(&name);
    }
    if matches!(
        segments[0],
        "status" | "detail" | "search" | "login" | "signup" | "tv" | "ajax" | "api"
    ) {
        return invalid("请填微博账号，不要贴单条微博或搜索页。");
    }
    if is_weibo_uid(segments[0]) {
        return finish_weibo(segments[0]);
    }
    weibo_name_hit(&percent_decode(segments[0]))
}

fn weibo_name_hit(name: &str) -> SourceHit {
    let name = name.trim().trim_start_matches('@');
    if !weibo_nickname(name) {
        return invalid("没找到这个微博账号");
    }
    SourceHit {
        kind: "weibo-name",
        supported: false,
        url: format!("https://m.weibo.cn/n/{}", name),
        handle: name.to_string(),
        message: String::new(),
    }
}

fn weibo_nickname(name: &str) -> bool {
    let count = name.chars().count();
    (1..=30).contains(&count)
        && !name.chars().any(|char| char.is_whitespace() || char.is_control() || char == '/' || char == '?' || char == '#')
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn is_weibo_uid(uid: &str) -> bool {
    (5..=16).contains(&uid.len()) && uid.chars().all(|char| char.is_ascii_digit())
}

fn finish_instagram(handle: &str) -> SourceHit {
    if !is_ig_handle(handle) {
        return invalid("Instagram 账号只能是 1–30 位字母、数字、下划线或点。");
    }
    if IG_RESERVED.contains(&handle.to_ascii_lowercase().as_str()) {
        return invalid("这是 Instagram 的页面，不是一个账号。");
    }
    let handle = handle.to_ascii_lowercase();
    SourceHit {
        kind: "instagram",
        supported: true,
        url: format!("https://www.instagram.com/{handle}/"),
        handle,
        message: String::new(),
    }
}

fn is_x_handle(handle: &str) -> bool {
    let count = handle.chars().count();
    (1..=15).contains(&count)
        && handle
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_ig_handle(handle: &str) -> bool {
    let count = handle.chars().count();
    if !(1..=30).contains(&count) {
        return false;
    }
    if handle.starts_with('.') || handle.ends_with('.') || handle.contains("..") {
        return false;
    }
    handle
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

fn invalid(message: &str) -> SourceHit {
    SourceHit {
        kind: "invalid",
        supported: false,
        handle: String::new(),
        url: String::new(),
        message: message.to_string(),
    }
}

fn unsupported(kind: &'static str, message: &str) -> SourceHit {
    SourceHit {
        kind,
        supported: false,
        handle: String::new(),
        url: String::new(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DiscordAdapter, FeedAdapter, InstagramAdapter, ThreadsAdapter, XAdapter, canon_platform,
        home_url, login_page, login_url, parse_source, parse_source_for, WEIBO_DESKTOP_LOGIN,
        WEIBO_IOS_LOGIN,
    };

    #[test]
    fn x_handles_and_profile_urls() {
        for input in [
            "@Desk_Sample",
            "desk_sample",
            "https://x.com/Desk_Sample?s=21",
            "https://twitter.com/desk_sample/",
        ] {
            let hit = parse_source(input);
            assert!(hit.supported, "{input}");
            assert_eq!(hit.kind, "x");
            assert_eq!(hit.handle, "desk_sample");
            assert_eq!(hit.url, "https://x.com/desk_sample");
        }
        assert_eq!(
            XAdapter.profile_url("@desk_sample").as_deref(),
            Some("https://x.com/desk_sample")
        );
    }

    #[test]
    fn threads_and_instagram_urls_and_prefixes() {
        let threads = parse_source("threads:@Sample.Desk");
        assert!(threads.supported);
        assert_eq!(threads.kind, "threads");
        assert_eq!(threads.handle, "sample.desk");
        assert_eq!(threads.url, "https://www.threads.net/@sample.desk");
        assert_eq!(
            parse_source("https://www.threads.net/@sampledesk").url,
            "https://www.threads.net/@sampledesk"
        );
        assert!(!parse_source("https://www.threads.net/@sampledesk/post/AbC12").supported);

        let ig = parse_source_for("Sample.Name", "instagram");
        assert_eq!(ig.kind, "instagram");
        assert_eq!(ig.url, "https://www.instagram.com/sample.name/");
        assert_eq!(
            parse_source("https://www.instagram.com/sample.name/").handle,
            "sample.name"
        );
        assert!(!parse_source("https://www.instagram.com/p/AbCdEf/").supported);
        assert!(InstagramAdapter.supported());
        assert!(ThreadsAdapter.supported());
        assert_eq!(canon_platform(""), "x");
        assert_eq!(login_url("threads"), "https://www.threads.net/login");
        assert_eq!(login_page("threads", "ios"), "https://www.threads.com/login");
        assert_eq!(login_page("threads", "macos"), "https://www.threads.net/login");
        assert_eq!(login_page("instagram", "ios"), "https://www.instagram.com/accounts/login/");
        assert_eq!(home_url("instagram"), "https://www.instagram.com/");
        assert_eq!(login_url("weibo"), "https://m.weibo.cn/login");
        assert_eq!(login_page("weibo", "ios"), WEIBO_IOS_LOGIN);
        assert!(login_page("weibo", "ios").contains("entry=wapsso"));
        assert_eq!(login_page("weibo", "macos"), WEIBO_DESKTOP_LOGIN);
        assert!(login_page("weibo", "macos").contains("url=https%3A%2F%2Fm.weibo.cn%2F"));
        assert_eq!(login_page("x", "macos"), "https://x.com/login");
    }

    #[test]
    fn weibo_urls_uids_and_nicknames() {
        use super::{read_weibo_input, weibo_uid_in_url, WeiboInput};
        match read_weibo_input("https://weibo.com/u/1319518957") {
            WeiboInput::Uid { uid, .. } => assert_eq!(uid, "1319518957"),
            other => panic!("url {other:?}"),
        }
        match read_weibo_input("https://weibo.com/1319518957") {
            WeiboInput::Uid { uid, .. } => assert_eq!(uid, "1319518957"),
            other => panic!("bare uid url {other:?}"),
        }
        match read_weibo_input("https://m.weibo.cn/profile/1319518957") {
            WeiboInput::Uid { uid, .. } => assert_eq!(uid, "1319518957"),
            other => panic!("profile {other:?}"),
        }
        match read_weibo_input("https://m.weibo.cn/u/1319518957") {
            WeiboInput::Uid { uid, .. } => assert_eq!(uid, "1319518957"),
            other => panic!("mobile {other:?}"),
        }
        assert_eq!(weibo_uid_in_url("https://weibo.com/u/1319518957"), "1319518957");
        match read_weibo_input("1319518957") {
            WeiboInput::Uid { uid, .. } => assert_eq!(uid, "1319518957"),
            other => panic!("digits {other:?}"),
        }
        match read_weibo_input("https://weibo.com/n/%E6%8A%95%E6%98%9F%E8%B5%84%E4%BA%A7") {
            WeiboInput::Name { query } => assert_eq!(query, "投星资产"),
            other => panic!("encoded nick {other:?}"),
        }
        match read_weibo_input("https://m.weibo.cn/n/投星资产") {
            WeiboInput::Name { query } => assert_eq!(query, "投星资产"),
            other => panic!("nick url {other:?}"),
        }
        match read_weibo_input("投星资产") {
            WeiboInput::Name { query } => assert_eq!(query, "投星资产"),
            other => panic!("nick {other:?}"),
        }
        match read_weibo_input("https://m.weibo.cn/detail/5234567890") {
            WeiboInput::Reject { message } => assert!(message.contains("账号")),
            other => panic!("post {other:?}"),
        }
    }

    #[test]
    fn rejects_posts_reserved_words_and_junk() {
        assert!(!parse_source("https://x.com/desk_sample/status/1").supported);
        assert!(!parse_source("@home").supported);
        assert!(!parse_source("has spaces").supported);
        assert!(!parse_source("").supported);
        assert!(!parse_source("$NVDA").supported);
    }

    #[test]
    fn discord_is_recognized_but_not_loaded() {
        let discord = parse_source("https://discord.com/channels/100/200");
        assert_eq!(discord.kind, "discord");
        assert!(!discord.supported);
        assert_eq!(DiscordAdapter.id(), "discord");
        assert!(!DiscordAdapter.supported());
    }
}
