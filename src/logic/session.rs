//! Whether a home-page probe means the user is signed in.
//!
//! Signed in only when the page stayed on that platform's home and the probe
//! saw a logged-in marker (compose box, account switcher, or inbox). A login
//! URL is signed out. Home without a marker is still loading (`pending`).
//! Weibo's home and `/u/<uid>` are public, and a guest still gets SUB cookies,
//! so those pages never count. Weibo login is `m.weibo.cn/api/config`.

/// `in`, `out`, or `pending`. `platform` is `x`, `threads`, `instagram`, or `weibo`.
///
/// `out` is only a login URL. Home with a logged-in marker is `in`. Anything
/// else, including a home page that has not drawn a marker yet, is `pending`:
/// the saved session stays as it was.
pub fn classify_session(platform: &str, url: &str, has_marker: bool) -> &'static str {
    probe_verdict(platform, url, has_marker, false, false)
}

/// `in` when a logged-in marker or a viewer id was seen, `out` only for a
/// login URL or a login wall, otherwise `pending`.
pub fn probe_verdict(
    platform: &str,
    url: &str,
    has_marker: bool,
    login_wall: bool,
    viewer: bool,
) -> &'static str {
    let path = path_of(url);
    let login = match super::source::canon_platform(platform) {
        "threads" => threads_login(&path),
        "instagram" => instagram_login(&path),
        "weibo" => weibo_login(&path),
        _ => x_login(&path),
    };
    if login_wall || login {
        return "out";
    }
    let home = match super::source::canon_platform(platform) {
        "threads" => threads_home(&path),
        "instagram" => instagram_home(&path),
        // Weibo's `/` and `/u/` are public. A marker there is not a session.
        "weibo" => false,
        _ => x_home(&path),
    };
    if viewer || (home && has_marker) {
        "in"
    } else {
        "pending"
    }
}

/// `Some` when the probe should replace the saved session. `pending` does not.
pub fn session_update(verdict: &str) -> Option<&'static str> {
    match verdict {
        "in" => Some("in"),
        "out" => Some("out"),
        _ => None,
    }
}

/// What a scraper message says about the account that is logged in.
///
/// `Some("in" | "out" | "blocked")` only for a real session signal.
/// Public posts, `weibo-api`, and an Instagram profile with no viewer stay
/// `None`: the saved status is left as it is.
pub fn session_from_scrape(message: &str) -> Option<&'static str> {
    let value: serde_json::Value = serde_json::from_str(message).ok()?;
    let kind = value
        .get("type")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let state = value
        .get("state")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    match kind.as_str() {
        "session" if state == "signed-out" => Some("out"),
        "session" if state == "blocked" => Some("blocked"),
        "session" if state == "signed-in" => Some("in"),
        "ig-api" => instagram_viewer(value.get("raw").and_then(|item| item.as_str()).unwrap_or("")),
        _ => None,
    }
}

/// `data.login` from `https://m.weibo.cn/api/config`.
/// `true` is signed in, `false` is signed out, anything else is unknown.
pub fn weibo_config_state(raw: &str) -> Option<&'static str> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    match value.pointer("/data/login").and_then(|item| item.as_bool()) {
        Some(true) => Some("in"),
        Some(false) => Some("out"),
        None => None,
    }
}

/// `web_profile_info` is public. Only an actual viewer id proves a session.
fn instagram_viewer(raw: &str) -> Option<&'static str> {
    if raw.is_empty() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let id = value
        .pointer("/data/viewer/id")
        .or_else(|| value.pointer("/viewer/id"))
        .or_else(|| value.get("viewerId"));
    let proved = match id {
        Some(serde_json::Value::String(text)) => {
            let text = text.trim();
            !text.is_empty() && text != "0" && text != "null"
        }
        Some(serde_json::Value::Number(number)) => number.as_u64().unwrap_or(0) > 0,
        _ => false,
    };
    proved.then_some("in")
}

fn x_login(path: &str) -> bool {
    path.contains("/login") || path.contains("/i/flow/")
}

fn x_home(path: &str) -> bool {
    path == "/home" || path == "/home/"
}

fn threads_login(path: &str) -> bool {
    path.contains("/login") || path.contains("/accounts/login")
}

fn threads_home(path: &str) -> bool {
    path == "/" || path == "/home" || path == "/home/"
}

fn instagram_login(path: &str) -> bool {
    path.contains("/accounts/login") || path == "/login" || path == "/login/"
}

fn instagram_home(path: &str) -> bool {
    path == "/" || path == "/home" || path == "/home/"
}

fn weibo_login(path: &str) -> bool {
    path.contains("/login") || path.contains("/signin")
}

fn path_of(url: &str) -> String {
    let rest = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let after_host = rest.split_once('/').map(|(_, path)| path).unwrap_or("");
    let path = after_host.split(['?', '#']).next().unwrap_or("");
    if path.is_empty() {
        "/".into()
    } else {
        format!("/{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_session, session_from_scrape, weibo_config_state};

    #[test]
    fn x_home_and_a_marker_means_signed_in() {
        assert_eq!(classify_session("x", "https://x.com/home", true), "in");
        assert_eq!(classify_session("x", "https://x.com/home/", true), "in");
        assert_eq!(
            classify_session("x", "https://twitter.com/home?lang=en", true),
            "in"
        );
    }

    #[test]
    fn home_without_a_marker_is_still_loading() {
        assert_eq!(
            classify_session("x", "https://x.com/home", false),
            "pending"
        );
        assert_eq!(
            classify_session("threads", "https://www.threads.net/", false),
            "pending"
        );
        assert_eq!(
            classify_session("instagram", "https://www.instagram.com/", false),
            "pending"
        );
    }

    #[test]
    fn each_platform_login_is_signed_out_and_home_with_marker_is_in() {
        assert_eq!(
            classify_session("x", "https://x.com/i/flow/login", true),
            "out"
        );
        assert_eq!(classify_session("x", "https://x.com/login", false), "out");
        assert_eq!(
            classify_session("threads", "https://www.threads.net/login", false),
            "out"
        );
        assert_eq!(
            classify_session("threads", "https://www.threads.net/", true),
            "in"
        );
        assert_eq!(
            classify_session(
                "instagram",
                "https://www.instagram.com/accounts/login/",
                true
            ),
            "out"
        );
        assert_eq!(
            classify_session("instagram", "https://www.instagram.com/", true),
            "in"
        );
        assert_eq!(
            classify_session("x", "https://x.com/desk_sample", true),
            "pending"
        );
    }

    #[test]
    fn threads_com_home_needs_a_real_signal_and_a_blank_probe_stays_pending() {
        use super::{probe_verdict, session_update};
        assert_eq!(
            classify_session("threads", "https://www.threads.com/", true),
            "in"
        );
        assert_eq!(
            classify_session("threads", "https://www.threads.com/", false),
            "pending"
        );
        assert_eq!(
            classify_session("threads", "https://www.threads.com/login", false),
            "out"
        );
        assert_eq!(
            classify_session("threads", "https://www.threads.net/", false),
            "pending"
        );
        assert_eq!(
            probe_verdict("threads", "https://www.threads.com/", false, true, false),
            "out"
        );
        assert_eq!(
            probe_verdict("threads", "https://www.threads.com/", false, false, true),
            "in"
        );
        assert_eq!(
            probe_verdict("x", "https://x.com/home", false, false, false),
            "pending"
        );
        assert_eq!(session_update("pending"), None);
        assert_eq!(session_update("in"), Some("in"));
        assert_eq!(session_update("out"), Some("out"));
        assert_eq!(
            probe_verdict("instagram", "https://www.instagram.com/", false, false, false),
            "pending"
        );
        assert_eq!(
            probe_verdict("weibo", "https://m.weibo.cn/", true, false, false),
            "pending"
        );
    }

    #[test]
    fn guest_weibo_api_posts_do_not_set_in() {
        let posts = r#"{"type":"weibo-api","state":"ok","posts":[{"id":"5234567890123456","text":"hello"}],"raw":"{\"ok\":1,\"data\":{\"cards\":[]}}"}"#;
        assert_eq!(session_from_scrape(posts), None);
        let page = r#"{"type":"posts","state":"ok","posts":[{"id":"1","text":"public"}]}"#;
        assert_eq!(session_from_scrape(page), None);
        let ig = r#"{"type":"ig-api","state":"ok","posts":[{"id":"AbCdEf"}],"raw":"{\"data\":{\"user\":{\"username\":\"guest\"}}}"}"#;
        assert_eq!(session_from_scrape(ig), None);
        assert_ne!(
            classify_session("weibo", "https://m.weibo.cn/", true),
            "in"
        );
        assert_ne!(
            classify_session("weibo", "https://m.weibo.cn/u/1234567890", true),
            "in"
        );
    }

    #[test]
    fn weibo_config_login_false_sets_out() {
        assert_eq!(
            weibo_config_state(r#"{"ok":1,"data":{"login":false,"uid":""}}"#),
            Some("out")
        );
        assert_eq!(
            session_from_scrape(r#"{"type":"session","state":"signed-out","posts":[]}"#),
            Some("out")
        );
    }

    #[test]
    fn weibo_config_login_true_sets_in() {
        assert_eq!(
            weibo_config_state(r#"{"ok":1,"data":{"login":true,"uid":"1234567890"}}"#),
            Some("in")
        );
        assert_eq!(
            session_from_scrape(r#"{"type":"session","state":"signed-in","posts":[]}"#),
            Some("in")
        );
        let ig = r#"{"type":"ig-api","raw":"{\"data\":{\"viewer\":{\"id\":\"42\"}}}"}"#;
        assert_eq!(session_from_scrape(ig), Some("in"));
    }
}
