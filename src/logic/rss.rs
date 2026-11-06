//! RSS 2.0 and Atom, plus OPML subscribe lists.
//!
//! A feed is one card. Its handle is the canonical feed URL. Entries become
//! the same posts the other platforms use: a title, body, images, and a link.
//! Full `content:encoded` or Atom `content` wins over a short summary.

use super::model::Post;
use super::time::parse_time_ms;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFeed {
    pub title: String,
    pub link: String,
    pub image: String,
    pub posts: Vec<Post>,
}

pub fn parse_feed(xml: &str) -> Result<ParsedFeed, String> {
    let xml = xml.trim();
    if xml.is_empty() {
        return Err("订阅没有内容。".into());
    }
    let lower = xml.to_ascii_lowercase();
    let atom = lower.contains("<feed") && (lower.contains("http://www.w3.org/2005/atom") || lower.contains("<entry"));
    let blocks = if atom {
        blocks_of(xml, "entry")
    } else if lower.contains("<item") || lower.contains("<rss") || lower.contains("<channel") {
        blocks_of(xml, "item")
    } else {
        return Err("这不是 RSS 或 Atom。".into());
    };
    if blocks.is_empty() {
        return Err("这份订阅里没有文章。".into());
    }
    let head = channel_head(xml);
    let mut posts = Vec::new();
    for block in blocks {
        if let Some(post) = entry_from(&block, atom) {
            if posts.iter().any(|have: &Post| have.id == post.id) {
                continue;
            }
            posts.push(post);
        }
        if posts.len() == 40 {
            break;
        }
    }
    if posts.is_empty() {
        return Err("这份订阅里没有能显示的文章。".into());
    }
    posts.sort_by_key(|post| std::cmp::Reverse(post.time_ms));
    Ok(ParsedFeed {
        title: plain(&head.title),
        link: head.link,
        image: head.image,
        posts,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpmlFeed {
    pub title: String,
    pub url: String,
    /// Folder path. Nested OPML outlines join with ` / `. Empty means no folder.
    pub folder: String,
}

pub fn parse_opml(xml: &str) -> Result<Vec<OpmlFeed>, String> {
    let xml = xml.trim();
    if xml.is_empty() || !xml.to_ascii_lowercase().contains("<opml") && !xml.to_ascii_lowercase().contains("xmlurl") {
        return Err("这不是 OPML。".into());
    }
    let mut feeds = Vec::new();
    let lower = xml.to_ascii_lowercase();
    let mut names: Vec<String> = Vec::new();
    // `true` when this open `<outline>` pushed a folder name.
    let mut opened: Vec<bool> = Vec::new();
    let mut i = 0;
    while i < xml.len() && feeds.len() < 200 {
        let next_open = lower[i..].find("<outline").map(|at| i + at);
        let next_close = lower[i..].find("</outline").map(|at| i + at);
        let open_first = match (next_open, next_close) {
            (Some(open), Some(close)) => open <= close,
            (Some(_), None) => true,
            _ => false,
        };
        if open_first {
            let start = next_open.unwrap_or(i);
            let end = xml[start..].find('>').map(|at| start + at).unwrap_or(xml.len().saturating_sub(1));
            let tag = &xml[start..=end.min(xml.len().saturating_sub(1))];
            let self_closing = tag.trim_end().ends_with("/>");
            let url = attr(tag, "xmlUrl").or_else(|| attr(tag, "xmlurl"));
            if let Some(url) = url.as_deref().and_then(canonical_feed_url) {
                if !feeds.iter().any(|have: &OpmlFeed| have.url.eq_ignore_ascii_case(&url)) {
                    let title = attr(tag, "title").or_else(|| attr(tag, "text")).unwrap_or_default();
                    feeds.push(OpmlFeed {
                        title: plain(&title),
                        url,
                        folder: names.join(" / "),
                    });
                }
                if !self_closing {
                    opened.push(false);
                }
            } else if !self_closing {
                let name = folder_label(&attr(tag, "text").or_else(|| attr(tag, "title")).unwrap_or_default());
                let pushed = !name.is_empty();
                if pushed {
                    names.push(name);
                }
                opened.push(pushed);
            }
            i = end.saturating_add(1);
            continue;
        }
        if let Some(start) = next_close {
            if let Some(pushed) = opened.pop() {
                if pushed {
                    names.pop();
                }
            }
            i = start + "</outline".len();
            continue;
        }
        break;
    }
    if feeds.is_empty() {
        return Err("这份 OPML 里没有订阅。".into());
    }
    Ok(feeds)
}

fn folder_label(raw: &str) -> String {
    let name = plain(raw).split_whitespace().collect::<Vec<_>>().join(" ");
    let name = name.trim();
    if name.is_empty() {
        return String::new();
    }
    name.chars().take(40).collect()
}

pub fn write_opml(feeds: &[(String, String, String)]) -> String {
    let mut body = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n  <head><title>Pulse</title></head>\n  <body>\n",
    );
    let mut loose = Vec::new();
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (title, url, folder) in feeds {
        let folder = folder.trim();
        if folder.is_empty() {
            loose.push((title.clone(), url.clone()));
            continue;
        }
        if let Some((_, items)) = groups.iter_mut().find(|(name, _)| name == folder) {
            items.push((title.clone(), url.clone()));
        } else {
            groups.push((folder.to_string(), vec![(title.clone(), url.clone())]));
        }
    }
    for (title, url) in loose {
        body.push_str(&feed_outline(&title, &url, 2));
    }
    for (folder, items) in groups {
        body.push_str(&format!("    <outline text=\"{}\" title=\"{}\">\n", xml_escape(&folder), xml_escape(&folder)));
        for (title, url) in items {
            body.push_str(&feed_outline(&title, &url, 3));
        }
        body.push_str("    </outline>\n");
    }
    body.push_str("  </body>\n</opml>\n");
    body
}

fn feed_outline(title: &str, url: &str, depth: usize) -> String {
    let pad = "  ".repeat(depth);
    format!(
        "{pad}<outline type=\"rss\" text=\"{}\" title=\"{}\" xmlUrl=\"{}\"/>\n",
        xml_escape(title),
        xml_escape(title),
        xml_escape(url)
    )
}

pub fn canonical_feed_url(raw: &str) -> Option<String> {
    let url = raw.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    if url.len() < 12 || url.len() > 500 || url.chars().any(|char| char.is_whitespace() || char.is_control()) {
        return None;
    }
    Some(url.to_string())
}

struct Head {
    title: String,
    link: String,
    image: String,
}

fn channel_head(xml: &str) -> Head {
    let window = xml.chars().take(12_000).collect::<String>();
    Head {
        title: tag_text(&window, "title").unwrap_or_default(),
        link: first_link(&window),
        image: first_image(&window),
    }
}

fn entry_from(block: &str, atom: bool) -> Option<Post> {
    let title = plain(&tag_text(block, "title").unwrap_or_default());
    let link = if atom { atom_link(block) } else { tag_text(block, "link").unwrap_or_default() };
    let link = plain(&link);
    let guid = tag_text(block, "guid")
        .or_else(|| tag_text(block, "id"))
        .unwrap_or_default();
    let when = tag_text(block, "pubDate")
        .or_else(|| tag_text(block, "published"))
        .or_else(|| tag_text(block, "updated"))
        .or_else(|| tag_text(block, "dc:date"))
        .unwrap_or_default();
    let html = tag_text(block, "content:encoded")
        .or_else(|| tag_text(block, "content"))
        .filter(|text| !plain(text).is_empty())
        .or_else(|| tag_text(block, "description"))
        .or_else(|| tag_text(block, "summary"))
        .unwrap_or_default();
    let mut images = images_in(&html);
    for extra in enclosures(block) {
        if images.len() == 4 {
            break;
        }
        if !images.iter().any(|have| have == &extra) {
            images.push(extra);
        }
    }
    let body = plain(&html);
    let body = if body == title { String::new() } else { body };
    if title.is_empty() && body.is_empty() && images.is_empty() {
        return None;
    }
    let key = if guid.trim().is_empty() { link.clone() } else { guid };
    if key.trim().is_empty() && title.is_empty() {
        return None;
    }
    let url = if link.starts_with("http") { link } else if key.starts_with("http") { key.clone() } else { String::new() };
    Some(Post {
        id: rss_id(&key, &url, &title),
        text: clip(&body, 6000),
        time_ms: feed_time(&when),
        url: clip(&url, 500),
        images,
        link_title: clip(&title, 180),
        link_url: clip(&url, 500),
        quote: None,
    })
}

fn rss_id(key: &str, url: &str, title: &str) -> String {
    format!("r{:016x}", fnv(&format!("{key}\n{url}\n{title}")))
}

fn fnv(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn feed_time(raw: &str) -> i64 {
    let raw = raw.trim();
    if raw.is_empty() {
        return 0;
    }
    if let Some(ms) = parse_time_ms(raw) {
        return ms;
    }
    parse_rfc822(raw).unwrap_or(0)
}

fn parse_rfc822(raw: &str) -> Option<i64> {
    let rest = raw.split_once(',').map(|(_, rest)| rest).unwrap_or(raw).trim();
    let mut parts = rest.split_whitespace();
    let day: u32 = parts.next()?.parse().ok()?;
    let month = month_num(parts.next()?)?;
    let year: i32 = parts.next()?.parse().ok()?;
    let clock = parts.next()?;
    if clock.len() < 8 {
        return None;
    }
    let zone = parts.next().unwrap_or("GMT");
    let offset = match zone {
        "GMT" | "UTC" | "UT" | "Z" => "Z".to_string(),
        value if value.len() == 5 && (value.starts_with('+') || value.starts_with('-')) => {
            format!("{}:{}", &value[..3], &value[3..])
        }
        _ => "Z".to_string(),
    };
    parse_time_ms(&format!(
        "{year:04}-{month:02}-{day:02}T{}{offset}",
        &clock[..8]
    ))
}

fn month_num(name: &str) -> Option<u32> {
    Some(match &name.to_ascii_lowercase()[..3.min(name.len())] {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    })
}

fn blocks_of(xml: &str, name: &str) -> Vec<String> {
    let lower = xml.to_ascii_lowercase();
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(&open) {
        let start = from + rel;
        let Some(rel_end) = lower[start..].find(&close) else {
            break;
        };
        let end = start + rel_end + close.len();
        out.push(xml[start..end].to_string());
        from = end;
        if out.len() == 40 {
            break;
        }
    }
    out
}

fn tag_text(block: &str, name: &str) -> Option<String> {
    let lower = block.to_ascii_lowercase();
    let name = name.to_ascii_lowercase();
    let needle = format!("<{name}");
    let start = lower.find(&needle)?;
    let after = block[start + needle.len()..].find('>')? + start + needle.len() + 1;
    let close = format!("</{name}>");
    let end = lower[after..].find(&close)? + after;
    Some(decode_cdata(&block[after..end]))
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let key = format!("{name}=\"");
    let name_l = name.to_ascii_lowercase();
    let key = if lower.contains(&key) {
        key
    } else {
        format!("{name_l}=\"")
    };
    let start = lower.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(decode_basic(&tag[start..end]))
}

fn atom_link(block: &str) -> String {
    let lower = block.to_ascii_lowercase();
    let mut from = 0;
    let mut fallback = String::new();
    while let Some(rel) = lower[from..].find("<link") {
        let start = from + rel;
        let end = block[start..].find('>').map(|at| start + at + 1).unwrap_or(block.len());
        let tag = &block[start..end];
        let href = attr(tag, "href").unwrap_or_default();
        let rel_attr = attr(tag, "rel").unwrap_or_default();
        if href.starts_with("http") && (rel_attr.is_empty() || rel_attr == "alternate") {
            return href;
        }
        if fallback.is_empty() && href.starts_with("http") {
            fallback = href;
        }
        from = end;
    }
    fallback
}

fn first_link(block: &str) -> String {
    tag_text(block, "link")
        .filter(|link| link.starts_with("http"))
        .or_else(|| atom_link(block).into())
        .unwrap_or_default()
}

fn first_image(block: &str) -> String {
    images_in(block).into_iter().next().unwrap_or_default()
}

fn enclosures(block: &str) -> Vec<String> {
    let lower = block.to_ascii_lowercase();
    let mut out = Vec::new();
    for name in ["enclosure", "media:content", "media:thumbnail"] {
        let mut from = 0;
        let needle = format!("<{name}");
        while let Some(rel) = lower[from..].find(&needle) {
            let start = from + rel;
            let end = block[start..].find('>').map(|at| start + at + 1).unwrap_or(block.len());
            let tag = &block[start..end];
            let url = attr(tag, "url").unwrap_or_default();
            let kind = attr(tag, "type").unwrap_or_default();
            if url.starts_with("https://") && (kind.is_empty() || kind.starts_with("image/") || looks_image(&url)) {
                if !out.contains(&url) {
                    out.push(url);
                }
            }
            from = end;
            if out.len() == 4 {
                return out;
            }
        }
    }
    out
}

fn images_in(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = lower[from..].find("<img") {
        let start = from + rel;
        let end = html[start..].find('>').map(|at| start + at + 1).unwrap_or(html.len());
        if let Some(src) = attr(&html[start..end], "src") {
            if src.starts_with("https://") && !out.contains(&src) {
                out.push(src);
            }
        }
        from = end;
        if out.len() == 4 {
            break;
        }
    }
    out
}

fn looks_image(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".gif", ".webp", ".avif"]
        .iter()
        .any(|ext| path.ends_with(ext))
}

fn decode_cdata(raw: &str) -> String {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix("<![CDATA[")
        .and_then(|rest| rest.strip_suffix("]]>"))
        .unwrap_or(trimmed);
    inner.trim().to_string()
}

fn plain(raw: &str) -> String {
    let mut without = String::new();
    let lower = raw.to_ascii_lowercase();
    let mut index = 0;
    while index < raw.len() {
        if lower[index..].starts_with("<script") || lower[index..].starts_with("<style") {
            let close = if lower[index..].starts_with("<script") {
                "</script>"
            } else {
                "</style>"
            };
            if let Some(end) = lower[index..].find(close) {
                index += end + close.len();
                continue;
            }
        }
        if raw[index..].starts_with('<') {
            let rest = &lower[index..];
            if rest.starts_with("<br")
                || rest.starts_with("</p")
                || rest.starts_with("</div")
                || rest.starts_with("</h")
                || rest.starts_with("<p")
            {
                without.push('\n');
            }
            if let Some(end) = raw[index..].find('>') {
                index += end + 1;
                continue;
            }
        }
        let ch = raw[index..].chars().next().unwrap_or(' ');
        without.push(ch);
        index += ch.len_utf8();
    }
    decode_basic(&without)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn decode_basic(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        if let Some(end) = rest.find(';') {
            let token = &rest[1..end];
            let ch = if let Some(num) = token.strip_prefix('#') {
                let code = if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    num.parse().ok()
                };
                code.and_then(char::from_u32)
            } else {
                Some(match token {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    "nbsp" => ' ',
                    _ => {
                        out.push('&');
                        rest = &rest[1..];
                        continue;
                    }
                })
            };
            if let Some(ch) = ch {
                out.push(ch);
            }
            rest = &rest[end + 1..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rss_prefers_encoded_content_and_keeps_the_figure() {
        let xml = r#"<?xml version="1.0"?>
        <rss version="2.0"><channel>
          <title>投星笔记</title>
          <link>https://example.com/</link>
          <item>
            <title>开市</title>
            <link>https://example.com/posts/1</link>
            <guid>https://example.com/posts/1</guid>
            <pubDate>Wed, 04 Nov 2026 12:00:00 GMT</pubDate>
            <description>短摘要</description>
            <content:encoded><![CDATA[<p>全文在这里</p><img src="https://cdn.example.com/a.jpg"/>]]></content:encoded>
          </item>
        </channel></rss>"#;
        let feed = parse_feed(xml).unwrap();
        assert_eq!(feed.title, "投星笔记");
        assert_eq!(feed.posts.len(), 1);
        assert_eq!(feed.posts[0].link_title, "开市");
        assert!(feed.posts[0].text.contains("全文在这里"));
        assert!(!feed.posts[0].text.contains("短摘要"));
        assert_eq!(feed.posts[0].images, vec!["https://cdn.example.com/a.jpg".to_string()]);
        assert!(feed.posts[0].time_ms > 0);
        assert!(feed.posts[0].id.starts_with('r'));
        assert_eq!(feed.posts[0].url, "https://example.com/posts/1");
    }

    #[test]
    fn atom_and_opml_round_trip() {
        let atom = r#"<?xml version="1.0"?>
        <feed xmlns="http://www.w3.org/2005/atom">
          <title>Atom 刊</title>
          <entry>
            <title>一条</title>
            <link rel="alternate" href="https://example.com/a"/>
            <id>tag:example.com,2026:a</id>
            <updated>2026-11-04T00:00:00Z</updated>
            <summary>只有摘要</summary>
          </entry>
        </feed>"#;
        let feed = parse_feed(atom).unwrap();
        assert_eq!(feed.posts[0].link_title, "一条");
        assert_eq!(feed.posts[0].text, "只有摘要");
        assert_eq!(feed.posts[0].url, "https://example.com/a");
        let opml = r#"<opml version="2.0"><body>
          <outline text="甲" xmlUrl="https://example.com/a.xml"/>
          <outline text="甲" xmlUrl="https://example.com/a.xml"/>
          <outline title="乙" xmlUrl="https://example.com/b.xml"/>
        </body></opml>"#;
        let list = parse_opml(opml).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "甲");
        assert_eq!(list[1].url, "https://example.com/b.xml");
        let written = write_opml(&[("甲".into(), list[0].url.clone(), String::new())]);
        assert!(written.contains("xmlUrl=\"https://example.com/a.xml\""));
        assert!(parse_opml("hello").is_err());
        let nested = r#"<opml version="2.0"><body>
          <outline text="新闻">
            <outline text="科技" title="科技">
              <outline text="HN" xmlUrl="https://news.example/rss"/>
            </outline>
          </outline>
          <outline text="散的" xmlUrl="https://loose.example/feed"/>
        </body></opml>"#;
        let grouped = parse_opml(nested).unwrap();
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].folder, "新闻 / 科技");
        assert_eq!(grouped[0].title, "HN");
        assert_eq!(grouped[1].folder, "");
        let exported = write_opml(&[
            ("HN".into(), grouped[0].url.clone(), "新闻 / 科技".into()),
            ("散的".into(), grouped[1].url.clone(), String::new()),
        ]);
        assert!(exported.contains("text=\"新闻 / 科技\""));
        assert!(exported.contains("xmlUrl=\"https://loose.example/feed\""));
        let again = parse_opml(&exported).unwrap();
        assert_eq!(again.iter().find(|feed| feed.title == "HN").unwrap().folder, "新闻 / 科技");
    }
}
