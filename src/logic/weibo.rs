//! Weibo's mobile container JSON (`getIndex?containerid=107603<uid>`).
//!
//! `created_at` is a real timestamp. Pics and a retweet become the bubble
//! and its quote. HTML in `text` is dropped.

use super::model::{Post, Quote};
use super::time::parse_time_ms;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Feed {
    pub posts: Vec<Post>,
    pub display_name: String,
    pub avatar: String,
}

pub fn parse_index(raw: &str, uid: &str) -> Feed {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return Feed::empty();
    };
    let data = value.get("data").unwrap_or(&value);
    let user = data.get("userInfo").unwrap_or(&Value::Null);
    let (mut display_name, mut avatar) = author_of(user);
    // Some getIndex payloads omit `userInfo` and only put the account on
    // the first mblog. A profile timeline's mblog.user is that account.
    if display_name.is_empty() || avatar.is_empty() {
        if let Some(fallback) = first_mblog_user(data) {
            let (name, photo) = author_of(fallback);
            if display_name.is_empty() {
                display_name = name;
            }
            if avatar.is_empty() {
                avatar = photo;
            }
        }
    }
    let mut posts = Vec::new();
    if let Some(cards) = data.get("cards").and_then(|item| item.as_array()) {
        for card in cards {
            let Some(mblog) = card.get("mblog") else {
                continue;
            };
            let Some(post) = post_from(mblog, uid) else {
                continue;
            };
            posts.push(post);
            if posts.len() == 20 {
                break;
            }
        }
    }
    Feed {
        posts,
        display_name,
        avatar,
    }
}

fn post_from(mblog: &Value, uid: &str) -> Option<Post> {
    let id = mblog
        .get("id")
        .and_then(|item| item.as_str().map(|s| s.to_string()).or_else(|| item.as_i64().map(|n| n.to_string())))
        .or_else(|| mblog.get("mid").and_then(|item| item.as_str().map(|s| s.to_string())))?;
    if id.is_empty() {
        return None;
    }
    let text = strip_html(&text_of(mblog.get("text")));
    let time_ms = weibo_time(&text_of(mblog.get("created_at")));
    let mut images = Vec::new();
    push_pics(&mut images, mblog.get("pics"));
    let quote = mblog.get("retweeted_status").and_then(|retweet| {
        let author = text_of(retweet.pointer("/user/screen_name"));
        let body = strip_html(&text_of(retweet.get("text")));
        if author.is_empty() && body.is_empty() {
            return None;
        }
        push_pics(&mut images, retweet.get("pics"));
        Some(Quote {
            author,
            text: body.chars().take(500).collect(),
            url: String::new(),
        })
    });
    if text.is_empty() && images.is_empty() && quote.is_none() {
        return None;
    }
    Some(Post {
        id: id.clone(),
        text: text.chars().take(2000).collect(),
        time_ms,
        url: format!("https://m.weibo.cn/detail/{id}"),
        images,
        link_title: String::new(),
        link_url: String::new(),
        quote,
    })
    .map(|post| {
        let _ = uid;
        post
    })
}

fn push_pics(images: &mut Vec<String>, pics: Option<&Value>) {
    let Some(pics) = pics.and_then(|item| item.as_array()) else {
        return;
    };
    for pic in pics {
        if images.len() == 4 {
            return;
        }
        let url = pic
            .pointer("/large/url")
            .and_then(|item| item.as_str())
            .or_else(|| pic.get("url").and_then(|item| item.as_str()))
            .unwrap_or("");
        let url = url.trim();
        if url.starts_with("https://") && !images.iter().any(|have| have == url) {
            images.push(url.to_string());
        }
    }
}

fn strip_html(raw: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for char in raw.chars() {
        if char == '<' {
            tag = true;
            continue;
        }
        if char == '>' {
            tag = false;
            continue;
        }
        if !tag {
            out.push(char);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `Wed Nov 04 12:00:00 +0800 2026` → epoch ms. Other shapes return 0.
pub fn weibo_time(raw: &str) -> i64 {
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() < 6 {
        return 0;
    }
    let month = match parts[1] {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return 0,
    };
    let day: u32 = parts[2].parse().unwrap_or(0);
    let year: i32 = parts[5].parse().unwrap_or(0);
    let clock = parts[3];
    if day == 0 || year < 2000 || clock.len() < 8 {
        return 0;
    }
    let iso = format!(
        "{year:04}-{month:02}-{day:02}T{}{}",
        clock,
        offset_iso(parts[4])
    );
    parse_time_ms(&iso).unwrap_or(0)
}

fn offset_iso(raw: &str) -> String {
    if raw.len() == 5 && (raw.starts_with('+') || raw.starts_with('-')) {
        format!("{}:{}", &raw[..3], &raw[3..])
    } else {
        "Z".into()
    }
}

fn text_of(value: Option<&Value>) -> String {
    value.and_then(|item| item.as_str()).unwrap_or("").trim().to_string()
}

fn author_of(user: &Value) -> (String, String) {
    (text_of(user.get("screen_name")), portrait_url(user))
}

fn portrait_url(user: &Value) -> String {
    for key in ["avatar_hd", "avatar_large", "profile_image_url"] {
        let url = https_portrait(&text_of(user.get(key)));
        if !url.is_empty() {
            return url;
        }
    }
    String::new()
}

/// Weibo still serves some portraits as `http://`. The card only keeps `https`.
pub fn https_portrait(url: &str) -> String {
    let url = url.trim();
    let url = if let Some(rest) = url.strip_prefix("http://") {
        if rest.contains("sinaimg.cn") || rest.contains("sinaimg.com") {
            format!("https://{rest}")
        } else {
            return String::new();
        }
    } else {
        url.to_string()
    };
    if url.starts_with("https://") { url } else { String::new() }
}

fn first_mblog_user(data: &Value) -> Option<&Value> {
    let cards = data.get("cards")?.as_array()?;
    for card in cards {
        if let Some(user) = card.pointer("/mblog/user") {
            if user.is_object() {
                return Some(user);
            }
        }
        if let Some(group) = card.get("card_group").and_then(|item| item.as_array()) {
            for child in group {
                if let Some(user) = child.pointer("/mblog/user") {
                    if user.is_object() {
                        return Some(user);
                    }
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundUser {
    pub uid: String,
    pub screen_name: String,
    pub avatar: String,
}

/// First user in a search payload whose `screen_name` equals `query`.
pub fn find_user(raw: &str, query: &str) -> Option<FoundUser> {
    let query = query.trim();
    if query.is_empty() || raw.trim().is_empty() {
        return None;
    }
    let value: Value = serde_json::from_str(raw).ok()?;
    let mut found = None;
    walk_user(&value, query, &mut found);
    found
}

fn walk_user(value: &Value, query: &str, found: &mut Option<FoundUser>) {
    if found.is_some() {
        return;
    }
    match value {
        Value::Array(items) => {
            for item in items {
                walk_user(item, query, found);
            }
        }
        Value::Object(object) => {
            if let Some(user) = user_from(object, query) {
                *found = Some(user);
                return;
            }
            for child in object.values() {
                walk_user(child, query, found);
                if found.is_some() {
                    return;
                }
            }
        }
        _ => {}
    }
}

fn user_from(object: &serde_json::Map<String, Value>, query: &str) -> Option<FoundUser> {
    let name = object.get("screen_name").and_then(|item| item.as_str())?.trim();
    if name != query {
        return None;
    }
    let uid = id_text(object.get("idstr"))
        .or_else(|| id_text(object.get("id")))
        .or_else(|| id_text(object.get("uid")))?;
    if !(5..=16).contains(&uid.len()) || !uid.chars().all(|char| char.is_ascii_digit()) {
        return None;
    }
    let avatar = portrait_url(&Value::Object(object.clone()));
    Some(FoundUser {
        uid,
        screen_name: name.to_string(),
        avatar,
    })
}

fn id_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

impl Feed {
    fn empty() -> Self {
        Self {
            posts: Vec::new(),
            display_name: String::new(),
            avatar: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_index, weibo_time};

    #[test]
    fn container_json_keeps_time_photo_and_retweet() {
        let raw = r#"{
          "ok": 1,
          "data": {
            "userInfo": { "screen_name": "台账", "avatar_hd": "https://tvax1.sinaimg.cn/a.jpg" },
            "cards": [{ "card_type": 9, "mblog": {
              "id": "4999887766",
              "created_at": "Wed Nov 04 12:00:00 +0800 2026",
              "text": "你好 <a href=\"/\">微博</a>",
              "pics": [{ "url": "https://wx1.sinaimg.cn/orj360/abc.jpg" }],
              "retweeted_status": { "text": "原帖", "user": { "screen_name": "别人" } }
            }}]
          }
        }"#;
        let feed = parse_index(raw, "4999");
        assert_eq!(feed.display_name, "台账");
        assert_eq!(feed.posts.len(), 1);
        assert_eq!(feed.posts[0].text, "你好 微博");
        assert!(feed.posts[0].time_ms > 0);
        assert!(feed.posts[0].images[0].contains("sinaimg"));
        assert_eq!(feed.posts[0].quote.as_ref().unwrap().author, "别人");
        assert_eq!(weibo_time("Wed Nov 04 12:00:00 +0800 2026"), feed.posts[0].time_ms);
        assert!(feed.avatar.contains("sinaimg"));
    }

    #[test]
    fn a_missing_userinfo_uses_the_first_mblog_user() {
        let raw = r#"{
          "ok": 1,
          "data": {
            "cards": [{ "card_type": 9, "mblog": {
              "id": "4999887766",
              "created_at": "Wed Nov 04 12:00:00 +0800 2026",
              "text": "正文",
              "user": {
                "id": 2014433131,
                "screen_name": "台账",
                "profile_image_url": "http://tvax1.sinaimg.cn/crop.1/a.jpg"
              }
            }}]
          }
        }"#;
        let feed = parse_index(raw, "2014433131");
        assert_eq!(feed.display_name, "台账");
        assert_eq!(feed.avatar, "https://tvax1.sinaimg.cn/crop.1/a.jpg");
        assert_eq!(feed.posts.len(), 1);
    }

    #[test]
    fn search_takes_the_exact_screen_name_and_misses_the_rest() {
        use super::find_user;
        let raw = r#"{
          "ok": 1,
          "data": { "cards": [{ "card_group": [
            { "user": { "id": 1, "screen_name": "别人", "avatar_hd": "https://tva.sinaimg.cn/b.jpg" } },
            { "user": { "id": 1319518957, "screen_name": "投星资产", "avatar_hd": "https://tva.sinaimg.cn/a.jpg" } }
          ]}] }
        }"#;
        let hit = find_user(raw, "投星资产").unwrap();
        assert_eq!(hit.uid, "1319518957");
        assert_eq!(hit.screen_name, "投星资产");
        assert!(hit.avatar.contains("sinaimg"));
        assert!(find_user(raw, "不存在的名字").is_none());
        assert!(find_user("", "投星资产").is_none());
    }
}
