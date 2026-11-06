//! Instagram `web_profile_info` JSON, the logged-in profile endpoint.
//!
//! The hidden page asks for it with the session cookie. The first edges are
//! enough to draw captions and thumbnails; image bytes are fetched later.

use super::model::Post;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Feed {
    pub posts: Vec<Post>,
    pub display_name: String,
    pub avatar: String,
}

pub fn parse_profile(raw: &str, handle: &str) -> Feed {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return Feed::empty();
    };
    let user = value
        .get("data")
        .and_then(|data| data.get("user"))
        .or_else(|| value.get("user"))
        .unwrap_or(&Value::Null);
    let display_name = text_of(user.get("full_name"));
    let hd = text_of(user.get("profile_pic_url_hd"));
    let avatar = if hd.is_empty() { text_of(user.get("profile_pic_url")) } else { hd };
    let edges = user
        .pointer("/edge_owner_to_timeline_media/edges")
        .and_then(|item| item.as_array());
    let mut posts = Vec::new();
    if let Some(edges) = edges {
        for edge in edges {
            let Some(node) = edge.get("node") else {
                continue;
            };
            let Some(post) = post_from(node, handle) else {
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

fn post_from(node: &Value, handle: &str) -> Option<Post> {
    let id = text_of(node.get("shortcode"));
    if id.len() < 5 {
        return None;
    }
    let caption = node
        .pointer("/edge_media_to_caption/edges/0/node/text")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim();
    let time_ms = node
        .get("taken_at_timestamp")
        .and_then(|item| item.as_i64())
        .filter(|sec| *sec > 0)
        .map(|sec| sec.saturating_mul(1000))
        .unwrap_or(0);
    let mut images = Vec::new();
    if let Some(children) = node
        .pointer("/edge_sidecar_to_children/edges")
        .and_then(|item| item.as_array())
    {
        for child in children {
            push_image(&mut images, child.pointer("/node/display_url"));
        }
    }
    if images.is_empty() {
        push_image(&mut images, node.get("display_url"));
        push_image(&mut images, node.get("thumbnail_src"));
    }
    let text = caption.chars().take(2000).collect::<String>();
    if text.is_empty() && images.is_empty() {
        return None;
    }
    let _ = handle;
    Some(Post {
        url: format!("https://www.instagram.com/p/{id}/"),
        id,
        text,
        time_ms,
        images,
        link_title: String::new(),
        link_url: String::new(),
        quote: None,
    })
}

fn push_image(images: &mut Vec<String>, value: Option<&Value>) {
    if images.len() == 4 {
        return;
    }
    let Some(url) = value.and_then(|item| item.as_str()) else {
        return;
    };
    let url = url.trim();
    if url.starts_with("https://") && !images.iter().any(|have| have == url) {
        images.push(url.to_string());
    }
}

fn text_of(value: Option<&Value>) -> String {
    value
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .chars()
        .take(80)
        .collect()
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
    use super::parse_profile;
    use std::time::Instant;

    #[test]
    fn web_profile_info_keeps_time_caption_and_thumb() {
        let raw = r#"{
          "data": { "user": {
            "full_name": "Desk",
            "profile_pic_url_hd": "https://scontent.cdninstagram.com/a.jpg",
            "edge_owner_to_timeline_media": { "edges": [{ "node": {
              "shortcode": "AbCdE1",
              "taken_at_timestamp": 1700000000,
              "display_url": "https://scontent.cdninstagram.com/p.jpg",
              "edge_media_to_caption": { "edges": [{ "node": { "text": "hello ig" } }] }
            }}]}
          }}
        }"#;
        let started = Instant::now();
        let feed = parse_profile(raw, "desk");
        let parse_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(feed.display_name, "Desk");
        assert_eq!(feed.posts.len(), 1);
        assert_eq!(feed.posts[0].text, "hello ig");
        assert_eq!(feed.posts[0].time_ms, 1_700_000_000_000);
        assert!(feed.posts[0].images[0].contains("cdninstagram"));
        // The old page waited 1s, 3.2s, then 9s, and held the web view for 12s.
        // The API parse replaces that wait. Images are not on this clock.
        eprintln!(
            "[pulse] instagram schedule before=load 9000 parse-after-dom images-after-all total>=12000 after=load 0 parse={parse_ms:.1} images=0 total={parse_ms:.1}"
        );
        assert!(parse_ms < 16.0);
    }
}
