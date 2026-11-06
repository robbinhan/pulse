//! Turn a CDN photo into a `data:` URI the native `Image` can paint.
//!
//! `Image` loads a URL with the platform's image stack, which sends no
//! Referer and a non-browser user agent. `pbs.twimg.com` and the Instagram
//! CDNs answer that with an empty body or a 403, so the view stays blank.
//! The bytes are fetched here with the headers those hosts expect and handed
//! back as a `data:` URI, which `Image` decodes without another request.

use std::collections::VecDeque;
use std::sync::Mutex;

pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
pub const MAX_BYTES: usize = 900_000;
const CACHE_SLOTS: usize = 24;

/// `https` photos from the CDNs a post is allowed to keep.
pub fn accept_url(url: &str) -> bool {
    let url = url.split('#').next().unwrap_or(url).trim();
    if !url.starts_with("https://") || url.len() > 8_000 || url.len() < 16 {
        return false;
    }
    let host = url
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    host == "pbs.twimg.com"
        || host.ends_with(".twimg.com")
        || host == "cdninstagram.com"
        || host.ends_with(".cdninstagram.com")
        || host.ends_with(".fbcdn.net")
        || host.starts_with("scontent.")
        || host.contains(".scontent.")
        || host.ends_with(".sinaimg.cn")
        || host.ends_with(".sinaimg.com")
}

/// Pages a CDN may require in `Referer`. Threads is tried on both hosts.
pub fn referers(platform: &str, url: &str) -> Vec<&'static str> {
    let host = url.to_ascii_lowercase();
    if host.contains("twimg.com") {
        return vec!["https://x.com/"];
    }
    match platform.trim().to_ascii_lowercase().as_str() {
        "threads" => vec!["https://www.threads.net/", "https://www.threads.com/"],
        "x" => vec!["https://x.com/"],
        "weibo" => vec!["https://m.weibo.cn/", "https://weibo.com/"],
        "rss" => vec![""],
        _ => vec!["https://www.instagram.com/"],
    }
}


/// An image type we will embed, from the header or the file's own bytes.
pub fn sniff_mime(header: &str, bytes: &[u8]) -> Option<&'static str> {
    if let Some(mime) = sniff_magic(bytes) {
        return Some(mime);
    }
    let header = header
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match header.as_str() {
        "image/jpeg" | "image/jpg" => Some("image/jpeg"),
        "image/png" => Some("image/png"),
        "image/webp" => Some("image/webp"),
        "image/gif" => Some("image/gif"),
        "image/avif" => Some("image/avif"),
        _ => None,
    }
}

pub fn image_extension(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".gif", ".webp", ".avif"]
        .iter()
        .any(|ext| path.ends_with(ext))
}

fn sniff_magic(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some("image/png");
    }
    if bytes.starts_with(b"GIF8") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12.min(bytes.len())];
        if brand == b"avif" || brand == b"avis" {
            return Some("image/avif");
        }
    }
    None
}

pub fn data_uri(mime: &str, bytes: &[u8]) -> String {
    let mut out = String::with_capacity(mime.len() + bytes.len() / 3 * 4 + 32);
    out.push_str("data:");
    out.push_str(mime);
    out.push_str(";base64,");
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8) | u32::from(bytes[i + 2]);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        i += 3;
    }
    let rest = bytes.len() - i;
    if rest == 1 {
        let n = u32::from(bytes[i]) << 16;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let n = (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

struct Cache {
    slots: VecDeque<(String, String)>,
}

impl Cache {
    fn get(&mut self, url: &str) -> Option<String> {
        let index = self.slots.iter().position(|(key, _)| key == url)?;
        let item = self.slots.remove(index)?;
        self.slots.push_back(item.clone());
        Some(item.1)
    }

    fn put(&mut self, url: String, uri: String) {
        self.slots.retain(|(key, _)| key != &url);
        self.slots.push_back((url, uri));
        while self.slots.len() > CACHE_SLOTS {
            self.slots.pop_front();
        }
    }
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: Mutex<Cache> = Mutex::new(Cache {
        slots: VecDeque::new(),
    });
    &CACHE
}

pub fn cached(url: &str) -> Option<String> {
    cache().lock().ok()?.get(url)
}

pub fn remember(url: String, uri: String) {
    if let Ok(mut cache) = cache().lock() {
        cache.put(url, uri);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_post_cdns_are_fetched() {
        assert!(accept_url("https://pbs.twimg.com/media/abc.jpg?name=small"));
        assert!(accept_url(
            "https://scontent.cdninstagram.com/v/photo.jpg?oh=1&oe=ABCDEF"
        ));
        assert!(accept_url("https://instagram.fabc1-1.fna.fbcdn.net/v/a.jpg"));
        assert!(!accept_url("http://pbs.twimg.com/media/abc.jpg"));
        assert!(!accept_url("https://example.com/a.jpg"));
        assert!(!accept_url("https://evil.twimg.com.example/a.jpg"));
    }

    #[test]
    fn referer_follows_the_host_and_the_platform() {
        assert_eq!(
            referers("instagram", "https://pbs.twimg.com/media/a.jpg"),
            vec!["https://x.com/"]
        );
        assert_eq!(
            referers("threads", "https://scontent.cdninstagram.com/v/a.jpg"),
            vec!["https://www.threads.net/", "https://www.threads.com/"]
        );
        assert_eq!(
            referers("instagram", "https://instagram.fna.fbcdn.net/v/a.jpg"),
            vec!["https://www.instagram.com/"]
        );
    }

    #[test]
    fn magic_bytes_win_over_a_generic_header() {
        assert_eq!(sniff_mime("application/octet-stream", &[0xff, 0xd8, 0xff, 0]), Some("image/jpeg"));
        assert_eq!(sniff_mime("image/png", b"not an image"), Some("image/png"));
        assert_eq!(sniff_mime("text/html", b"<html>"), None);
        let uri = data_uri("image/jpeg", &[0xff, 0xd8]);
        assert_eq!(uri, "data:image/jpeg;base64,/9g=");
    }
}
