//! Posts embedded in a Threads or Instagram page.
//!
//! Those sites put the timeline in `<script type="application/json">` blocks
//! long before the DOM has a post link. The objects we want carry `code` or
//! `shortcode`, `taken_at`, `caption.text`, `image_versions2.candidates[].url`,
//! `carousel_media`, and `video_versions`. The caption is never the visible
//! text of a `<time>` element.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedPost {
    pub id: String,
    pub text: String,
    pub time_ms: i64,
    pub url: String,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmbedFeed {
    pub posts: Vec<EmbedPost>,
    pub display_name: String,
    pub avatar: String,
}

/// Walks every JSON value in `scripts` and keeps posts for `handle`.
pub fn parse_scripts(scripts: &[String], platform: &str, handle: &str) -> EmbedFeed {
    let mut feed = EmbedFeed::default();
    let mut seen = HashSet::new();
    for script in scripts {
        let Ok(value) = serde_json::from_str::<Value>(script) else {
            continue;
        };
        walk(&value, platform, handle, &mut feed, &mut seen);
    }
    feed
}

/// `~/Library/Application Support/Pulse/dumps/<platform>-<handle>.html` under `home`.
pub fn dump_file(home: &Path, platform: &str, handle: &str, html: &str) -> Result<PathBuf, String> {
    let mut dir = PathBuf::from(home);
    dir.push("Library");
    dir.push("Application Support");
    dir.push("Pulse");
    dir.push("dumps");
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let name = format!("{}-{}.html", file_piece(platform), file_piece(handle));
    let path = dir.join(name);
    std::fs::write(&path, html).map_err(|err| err.to_string())?;
    Ok(path)
}

pub fn dump_enabled() -> bool {
    matches!(std::env::var("THEYSAY_DUMP"), Ok(value) if value == "1")
}

fn walk(
    value: &Value,
    platform: &str,
    handle: &str,
    feed: &mut EmbedFeed,
    seen: &mut HashSet<String>,
) {
    match value {
        Value::Object(map) => {
            note_profile(map, handle, feed);
            if let Some(post) = as_post(map, platform, handle) {
                if seen.insert(post.id.clone()) {
                    feed.posts.push(post);
                }
            }
            for child in map.values() {
                walk(child, platform, handle, feed, seen);
            }
        }
        Value::Array(items) => {
            for child in items {
                walk(child, platform, handle, feed, seen);
            }
        }
        _ => {}
    }
}

fn note_profile(map: &serde_json::Map<String, Value>, handle: &str, feed: &mut EmbedFeed) {
    let Some(user) = map.get("user").and_then(|value| value.as_object()) else {
        return;
    };
    let username = user
        .get("username")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if !handle.is_empty() && !username.eq_ignore_ascii_case(handle) {
        return;
    }
    if feed.display_name.is_empty() {
        if let Some(name) = user.get("full_name").and_then(|value| value.as_str()) {
            let name = name.trim();
            if !name.is_empty() && !name.eq_ignore_ascii_case(handle) {
                feed.display_name = name.chars().take(80).collect();
            }
        }
    }
    if feed.avatar.is_empty() {
        if let Some(url) = user.get("profile_pic_url").and_then(|value| value.as_str()) {
            if url.starts_with("https://") {
                feed.avatar = url.to_string();
            }
        }
    }
}

fn as_post(
    map: &serde_json::Map<String, Value>,
    platform: &str,
    handle: &str,
) -> Option<EmbedPost> {
    let id = code_of(map)?;
    let time_ms = time_ms_of(map).unwrap_or(0);
    let username = map
        .get("user")
        .and_then(|value| value.get("username"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if !handle.is_empty() && !username.is_empty() && !username.eq_ignore_ascii_case(handle) {
        return None;
    }
    if repost_of_someone_else(map) {
        return None;
    }
    let mut text = caption_of(map);
    if super::model::clock_text_only(&text) {
        text.clear();
    }
    let images = images_of(map);
    if text.is_empty() && images.is_empty() {
        return None;
    }
    let owner = if username.is_empty() { handle } else { username };
    Some(EmbedPost {
        url: post_url(platform, owner, &id),
        id,
        text,
        time_ms,
        images,
    })
}

fn repost_of_someone_else(map: &serde_json::Map<String, Value>) -> bool {
    map.get("text_post_app_info")
        .and_then(|value| value.get("share_info"))
        .and_then(|value| value.get("reposted_post"))
        .map(|value| !value.is_null())
        .unwrap_or(false)
}

fn code_of(map: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["code", "shortcode"] {
        if let Some(code) = map.get(key).and_then(|value| value.as_str()) {
            if is_code(code) {
                return Some(code.to_string());
            }
        }
    }
    None
}

fn is_code(code: &str) -> bool {
    (5..=32).contains(&code.len())
        && code.chars().any(|char| char.is_ascii_alphabetic())
        && code
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
}

fn time_ms_of(map: &serde_json::Map<String, Value>) -> Option<i64> {
    for key in ["taken_at", "taken_at_timestamp", "device_timestamp"] {
        if let Some(ms) = map.get(key).and_then(epoch_ms) {
            return Some(ms);
        }
    }
    if let Some(caption) = map.get("caption").and_then(|value| value.as_object()) {
        if let Some(ms) = caption.get("created_at").and_then(epoch_ms) {
            return Some(ms);
        }
    }
    map.get("created_at").and_then(epoch_ms)
}

/// Unix seconds or milliseconds, including JSON floats (`1729641600.0`) and ISO strings.
fn epoch_ms(value: &Value) -> Option<i64> {
    let raw = if let Some(raw) = value.as_i64() {
        raw
    } else if let Some(raw) = value.as_u64() {
        i64::try_from(raw).ok()?
    } else if let Some(raw) = value.as_f64() {
        if !raw.is_finite() || raw <= 0.0 {
            return None;
        }
        raw.round() as i64
    } else if let Some(text) = value.as_str() {
        if let Some(ms) = super::time::parse_time_ms(text) {
            return Some(ms);
        }
        text.trim().parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0)? as i64
    } else {
        return None;
    };
    if raw > 1_000_000_000_000 {
        Some(raw)
    } else if raw > 1_000_000_000 {
        Some(raw.saturating_mul(1000))
    } else {
        None
    }
}

fn caption_of(map: &serde_json::Map<String, Value>) -> String {
    let from_caption = match map.get("caption") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Object(object)) => object
            .get("text")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };
    if !from_caption.trim().is_empty() {
        return from_caption.trim().chars().take(2000).collect();
    }
    map.get("edge_media_to_caption")
        .and_then(|value| value.get("edges"))
        .and_then(|value| value.as_array())
        .and_then(|edges| edges.first())
        .and_then(|edge| edge.get("node"))
        .and_then(|node| node.get("text"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .chars()
        .take(2000)
        .collect()
}

fn images_of(map: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut images = Vec::new();
    push_image(&mut images, best_candidate(map.get("image_versions2")));
    if let Some(carousel) = map.get("carousel_media").and_then(|value| value.as_array()) {
        for item in carousel {
            push_image(&mut images, best_candidate(item.get("image_versions2")));
            if images.len() == 4 {
                break;
            }
        }
    }
    if images.is_empty() {
        if let Some(url) = map.get("display_url").and_then(|value| value.as_str()) {
            push_image(&mut images, Some(url.to_string()));
        }
    }
    images
}

fn push_image(images: &mut Vec<String>, url: Option<String>) {
    let Some(url) = url else {
        return;
    };
    if images.len() < 4 && !images.contains(&url) {
        images.push(url);
    }
}

/// One still from a media object. The signed query is kept. A 48 px avatar is not.
fn best_candidate(versions: Option<&Value>) -> Option<String> {
    let candidates = versions?.get("candidates")?.as_array()?;
    let mut best: Option<(i64, String)> = None;
    for candidate in candidates {
        let url = candidate.get("url").and_then(|value| value.as_str()).unwrap_or("");
        if !url.starts_with("https://") || url.len() > 8_000 {
            continue;
        }
        let width = candidate.get("width").and_then(|value| value.as_i64()).unwrap_or(0);
        if (1..=64).contains(&width) {
            continue;
        }
        let score = if width == 0 {
            0
        } else {
            4_000 - (width - 1080).abs()
        };
        if best.as_ref().map(|(have, _)| score > *have).unwrap_or(true) {
            best = Some((score, url.to_string()));
        }
    }
    best.map(|(_, url)| url)
}

fn post_url(platform: &str, handle: &str, code: &str) -> String {
    if platform == "instagram" {
        format!("https://www.instagram.com/p/{code}/")
    } else if handle.is_empty() {
        format!("https://www.threads.net/post/{code}")
    } else {
        format!("https://www.threads.net/@{handle}/post/{code}")
    }
}

fn file_piece(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_'))
        .take(40)
        .collect();
    if cleaned.is_empty() {
        "page".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> String {
        r#"{
          "require": [{
            "__bbox": {
              "result": {
                "data": {
                  "media": {
                    "user": {
                      "username": "kyung31_",
                      "full_name": "Kyung",
                      "profile_pic_url": "https://scontent.cdninstagram.com/v/avatar.jpg?_nc=1"
                    },
                    "thread_items": [
                      {
                        "post": {
                          "code": "DAabc12xy",
                          "taken_at": 1729641600,
                          "caption": { "text": "10/23/26" },
                          "user": { "username": "kyung31_", "full_name": "Kyung" },
                          "image_versions2": { "candidates": [
                            { "width": 48, "url": "https://scontent.cdninstagram.com/v/tiny.jpg" },
                            { "width": 640, "url": "https://scontent-nrt1-1.cdninstagram.com/v/t51/photo.jpg?stp=dst-jpg&_nc_ht=scontent" },
                            { "width": 1080, "url": "https://scontent-nrt1-1.cdninstagram.com/v/t51/photo-big.jpg?stp=dst-jpg&oh=signed" }
                          ]},
                          "carousel_media": [
                            { "image_versions2": { "candidates": [
                              { "width": 1080, "url": "https://scontent.cdninstagram.com/v/slide.jpg?sig=1" }
                            ]},
                              "video_versions": [
                                { "url": "https://scontent.cdninstagram.com/v/slide.mp4", "width": 720 }
                              ]
                            }
                          ],
                          "video_versions": [
                            { "url": "https://scontent.cdninstagram.com/v/clip.mp4", "width": 720 }
                          ]
                        }
                      },
                      {
                        "post": {
                          "code": "DBonlytime",
                          "taken_at": 1729728000,
                          "caption": { "text": "2d" },
                          "user": { "username": "kyung31_" },
                          "image_versions2": { "candidates": [] }
                        }
                      },
                      {
                        "post": {
                          "code": "DCrepost99",
                          "taken_at": 1729814400,
                          "caption": { "text": "someone else's words" },
                          "user": { "username": "other_user" },
                          "text_post_app_info": { "share_info": { "reposted_post": { "code": "DZother" } } }
                        }
                      },
                      {
                        "post": {
                          "code": "DDcaption1",
                          "taken_at": 1729900800,
                          "caption": { "text": "morning light on the desk" },
                          "user": { "username": "kyung31_", "full_name": "Kyung", "profile_pic_url": "https://scontent.cdninstagram.com/v/avatar.jpg?_nc=1" }
                        }
                      }
                    ]
                  }
                }
              }
            }
          }]
        }"#
        .to_string()
    }

    #[test]
    fn thread_items_keep_the_caption_and_the_signed_photo() {
        let feed = parse_scripts(&[fixture()], "threads", "kyung31_");
        assert_eq!(feed.display_name, "Kyung");
        assert!(feed.avatar.contains("avatar.jpg?_nc=1"));
        let ids: Vec<_> = feed.posts.iter().map(|post| post.id.as_str()).collect();
        assert_eq!(ids, ["DAabc12xy", "DDcaption1"]);
        let photo = &feed.posts[0];
        assert_eq!(photo.text, "");
        assert_eq!(photo.time_ms, 1_729_641_600_000);
        assert!(photo.images[0].contains("photo-big.jpg?stp=dst-jpg&oh=signed"));
        assert!(photo.images[1].contains("slide.jpg?sig=1"));
        assert!(photo.images.iter().all(|url| !url.ends_with(".mp4") && !url.contains("/tiny.jpg")));
        assert_eq!(photo.url, "https://www.threads.net/@kyung31_/post/DAabc12xy");
        assert_eq!(feed.posts[1].text, "morning light on the desk");
        assert!(feed.posts.iter().all(|post| post.text != "2d"));
    }

    #[test]
    fn an_instagram_grid_node_uses_its_caption_and_display_url() {
        let script = r#"{
          "edge_owner_to_timeline_media": {
            "edges": [{
              "node": {
                "shortcode": "AbCdEf1",
                "taken_at_timestamp": 1700000000,
                "display_url": "https://scontent.cdninstagram.com/v/grid.jpg?ig_cache_key=1",
                "edge_media_to_caption": { "edges": [{ "node": { "text": "grid caption" } }] },
                "owner": { "username": "sample.name" }
              }
            }]
          }
        }"#;
        let feed = parse_scripts(&[script.into()], "instagram", "sample.name");
        assert_eq!(feed.posts.len(), 1);
        assert_eq!(feed.posts[0].text, "grid caption");
        assert_eq!(
            feed.posts[0].images,
            vec!["https://scontent.cdninstagram.com/v/grid.jpg?ig_cache_key=1".to_string()]
        );
        assert_eq!(feed.posts[0].url, "https://www.instagram.com/p/AbCdEf1/");
        assert_eq!(feed.posts[0].time_ms, 1_700_000_000_000);
    }

    #[test]
    fn a_post_without_taken_at_is_kept_with_an_unknown_time() {
        let script = r#"{
          "shortcode": "NoTime1x",
          "caption": { "text": "no clock on this one" },
          "display_url": "https://scontent.cdninstagram.com/v/a.jpg",
          "user": { "username": "aartisehwag" }
        }"#;
        let feed = parse_scripts(&[script.into()], "instagram", "aartisehwag");
        assert_eq!(feed.posts.len(), 1);
        assert_eq!(feed.posts[0].id, "NoTime1x");
        assert_eq!(feed.posts[0].time_ms, 0);
        assert_eq!(feed.posts[0].text, "no clock on this one");
    }

    #[test]
    fn the_aartisehwag_profile_shell_has_no_invented_post_times() {
        let html = include_str!("testdata/instagram-aartisehwag.html");
        assert!(!html.contains("taken_at"));
        let mut scripts = Vec::new();
        let mut rest = html;
        while let Some(start) = rest.find("<script type=\"application/json\"") {
            let after = &rest[start..];
            let Some(open) = after.find('>') else { break };
            let body = &after[open + 1..];
            let Some(end) = body.find("</script>") else { break };
            scripts.push(body[..end].to_string());
            rest = &body[end + 9..];
        }
        assert!(scripts.len() > 10);
        let feed = parse_scripts(&scripts, "instagram", "aartisehwag");
        assert!(
            feed.posts.is_empty(),
            "this dump is the profile shell; the grid is not in the HTML"
        );
        assert!(feed.posts.iter().all(|post| post.time_ms == 0 || post.time_ms > 1_000_000_000_000));
    }

    #[test]
    fn a_float_taken_at_is_the_posts_real_time() {
        let script = r#"{
          "code": "ZzFloat1",
          "taken_at": 1729641600.0,
          "caption": { "text": "from a float" },
          "user": { "username": "aartisehwag" }
        }"#;
        let feed = parse_scripts(&[script.into()], "instagram", "aartisehwag");
        assert_eq!(feed.posts.len(), 1);
        assert_eq!(feed.posts[0].id, "ZzFloat1");
        assert_eq!(feed.posts[0].time_ms, 1_729_641_600_000);
    }

    #[test]
    fn a_dump_file_uses_the_pulse_support_directory() {
        let root = std::env::temp_dir().join(format!("pulse-dump-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = dump_file(&root, "threads", "kyung31_", "<html>page</html>").unwrap();
        assert!(path.ends_with("Library/Application Support/Pulse/dumps/threads-kyung31_.html"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "<html>page</html>");
        let sneaky = dump_file(&root, "../threads", "kyung31_", "x").unwrap();
        assert!(sneaky.ends_with("threads-kyung31_.html"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
