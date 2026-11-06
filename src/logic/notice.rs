//! Which new posts deserve a system notification.
//!
//! The first time an account has posts, the newest time is remembered and
//! nothing is sent. Later refreshes notify only for posts newer than that
//! mark. A post hidden by a mute keyword never produces a notification.
//! Turning notifications off still moves the mark forward, so enabling them
//! later does not announce the backlog.

use super::model::{self, Card, Post, Settings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub platform: String,
    pub handle: String,
    pub title: String,
    pub body: String,
    pub count: u32,
}

/// Updates `card.notify_ms` and returns one grouped notice when something new arrived.
pub fn consider_card(card: &mut Card, platform: &str, settings: &Settings) -> Option<Notice> {
    consider_known(card, platform, settings, &[])
}

/// Like [`consider_card`], and also treats an id that was not in `known_ids`
/// as new. The first batch (`notify_ms == 0`) still only seeds the watermark.
pub fn consider_known(
    card: &mut Card,
    platform: &str,
    settings: &Settings,
    known_ids: &[String],
) -> Option<Notice> {
    let (mark, notice) = plan(
        &card.posts,
        card.notify_ms,
        &settings.mute_keywords,
        settings.notify && platform_enabled(settings, platform),
        platform,
        &card.label,
        &card.handle,
        known_ids,
    );
    card.notify_ms = mark;
    notice
}

pub fn plan(
    posts: &[Post],
    watermark_ms: i64,
    keywords: &[String],
    enabled: bool,
    platform: &str,
    label: &str,
    handle: &str,
    known_ids: &[String],
) -> (i64, Option<Notice>) {
    let visible: Vec<&Post> = posts
        .iter()
        .filter(|post| !model::post_hidden(post, keywords))
        .collect();
    if visible.is_empty() {
        return (watermark_ms.max(0), None);
    }
    let newest = visible.iter().map(|post| post.time_ms).max().unwrap_or(0);
    // The first batch only seeds. Unknown times still count as "seen" so the
    // next new id can notify without pretending the post happened now.
    if watermark_ms <= 0 {
        return (newest.max(1), None);
    }
    let mut fresh: Vec<&Post> = visible
        .into_iter()
        .filter(|post| {
            post.time_ms > watermark_ms
                || (!known_ids.is_empty() && known_ids.iter().all(|id| id != &post.id))
        })
        .collect();
    let mark = newest.max(watermark_ms);
    if fresh.is_empty() || !enabled {
        return (mark, None);
    }
    fresh.sort_by_key(|post| post.time_ms);
    let newest_post = fresh[fresh.len() - 1];
    let count = fresh.len() as u32;
    (
        mark,
        Some(Notice {
            platform: platform.to_string(),
            handle: handle.to_string(),
            title: title(label, handle, platform),
            body: body(newest_post, count),
            count,
        }),
    )
}

pub fn platform_enabled(settings: &Settings, platform: &str) -> bool {
    match platform {
        "threads" => settings.notify_threads,
        "instagram" => settings.notify_instagram,
        "weibo" => settings.notify_weibo,
        "rss" => settings.notify_rss,
        _ => settings.notify_x,
    }
}

pub fn set_platform(settings: &mut Settings, platform: &str, on: bool) -> bool {
    match platform {
        "x" => settings.notify_x = on,
        "threads" => settings.notify_threads = on,
        "instagram" => settings.notify_instagram = on,
        "weibo" => settings.notify_weibo = on,
        "rss" => settings.notify_rss = on,
        _ => return false,
    }
    true
}

fn title(label: &str, handle: &str, platform: &str) -> String {
    let name = if label.trim().is_empty() {
        format!("@{handle}")
    } else {
        label.trim().to_string()
    };
    format!("{name} · {}", platform_label(platform))
}

fn platform_label(platform: &str) -> &'static str {
    match platform {
        "threads" => "Threads",
        "instagram" => "Instagram",
        "weibo" => "微博",
        "rss" => "RSS",
        _ => "X",
    }
}

fn body(post: &Post, count: u32) -> String {
    let caption = preview(&post.text);
    let one = if caption.is_empty() {
        if post.link_title.trim().is_empty() {
            "发布了新图片/视频".to_string()
        } else {
            preview(&post.link_title)
        }
    } else {
        caption
    };
    if count <= 1 {
        one
    } else {
        format!("{count} 条新帖：{one}")
    }
}

fn preview(text: &str) -> String {
    let trimmed = text.trim();
    let mut out: String = trimmed.chars().take(100).collect();
    if trimmed.chars().count() > 100 {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post(id: &str, time_ms: i64, text: &str) -> Post {
        Post {
            id: id.into(),
            text: text.into(),
            time_ms,
            url: format!("https://www.threads.net/@kyung/post/{id}"),
            ..Post::default()
        }
    }

    fn card(posts: Vec<Post>) -> Card {
        Card {
            handle: "kyung31_".into(),
            label: "Kyung".into(),
            posts: std::sync::Arc::new(posts),
            ..Card::default()
        }
    }

    #[test]
    fn the_first_batch_is_seeded_and_later_posts_notify_once() {
        let settings = Settings::default();
        let mut account = card(vec![post("Aaa111a", 1_000, "older"), post("Bbb222b", 2_000, "already here")]);
        assert!(consider_card(&mut account, "threads", &settings).is_none());
        assert_eq!(account.notify_ms, 2_000);

        std::sync::Arc::make_mut(&mut account.posts).push(post("Ccc333c", 3_000, "morning light on the desk"));
        std::sync::Arc::make_mut(&mut account.posts).push(post("Ddd444d", 4_000, "second"));
        let notice = consider_card(&mut account, "threads", &settings).unwrap();
        assert_eq!(notice.count, 2);
        assert_eq!(notice.title, "Kyung · Threads");
        assert!(notice.body.starts_with("2 条新帖："));
        assert!(notice.body.contains("second"));
        assert_eq!(account.notify_ms, 4_000);
        assert!(consider_card(&mut account, "threads", &settings).is_none());
    }

    #[test]
    fn a_picture_without_a_caption_uses_the_fallback_line() {
        let settings = Settings::default();
        let mut account = card(vec![post("Aaa111a", 1_000, "seed")]);
        consider_card(&mut account, "instagram", &settings);
        std::sync::Arc::make_mut(&mut account.posts).push(Post {
            id: "Pic11111".into(),
            text: String::new(),
            time_ms: 5_000,
            images: vec!["https://scontent.cdninstagram.com/v/a.jpg".into()],
            ..Post::default()
        });
        let notice = consider_card(&mut account, "instagram", &settings).unwrap();
        assert_eq!(notice.body, "发布了新图片/视频");
        assert_eq!(notice.title, "Kyung · Instagram");
    }

    #[test]
    fn blocked_words_and_a_disabled_switch_do_not_notify() {
        let mut settings = Settings::default();
        settings.mute_keywords = vec!["赠送".into()];
        let mut account = card(vec![post("Aaa111a", 1_000, "seed")]);
        consider_card(&mut account, "x", &settings);
        std::sync::Arc::make_mut(&mut account.posts).push(post("Bbb222b", 2_000, "限时赠送"));
        std::sync::Arc::make_mut(&mut account.posts).push(post("Ccc333c", 3_000, "ordinary"));
        let notice = consider_card(&mut account, "x", &settings).unwrap();
        assert_eq!(notice.count, 1);
        assert!(notice.body.contains("ordinary"));
        assert!(!notice.body.contains("赠送"));

        settings.notify = false;
        std::sync::Arc::make_mut(&mut account.posts).push(post("Ddd444d", 9_000, "while muted"));
        assert!(consider_card(&mut account, "x", &settings).is_none());
        assert_eq!(account.notify_ms, 9_000);
        settings.notify = true;
        std::sync::Arc::make_mut(&mut account.posts).push(post("Eee555e", 9_500, "after"));
        settings.notify_x = false;
        assert!(consider_card(&mut account, "x", &settings).is_none());
        settings.notify_x = true;
        std::sync::Arc::make_mut(&mut account.posts).push(post("Fff666f", 9_800, "threads still on"));
        let notice = consider_card(&mut account, "threads", &settings).unwrap();
        assert!(notice.title.ends_with("Threads"));
        assert_eq!(notice.handle, "kyung31_");
    }

    #[test]
    fn a_long_caption_stops_around_one_hundred_characters() {
        let settings = Settings::default();
        let mut account = card(vec![post("Aaa111a", 1_000, "seed")]);
        consider_card(&mut account, "threads", &settings);
        let long: String = "字".repeat(140);
        std::sync::Arc::make_mut(&mut account.posts).push(post("Lng11111", 2_000, &long));
        let notice = consider_card(&mut account, "threads", &settings).unwrap();
        assert!(notice.body.chars().count() <= 101);
        assert!(notice.body.ends_with('…'));
    }

    #[test]
    fn a_new_id_notifies_even_when_its_real_time_is_older_than_a_scrape_watermark() {
        let settings = Settings::default();
        let mut account = card(vec![post("Old11111", 1_000, "already saved")]);
        account.notify_ms = 1_731_000_000_000;
        let known = vec!["Old11111".into()];
        std::sync::Arc::make_mut(&mut account.posts).push(post("New22222", 1_700_000_000_000, "the real post"));
        let notice = consider_known(&mut account, "instagram", &settings, &known).unwrap();
        assert_eq!(notice.count, 1);
        assert!(notice.body.contains("the real post"));
        assert_eq!(account.notify_ms, 1_731_000_000_000);
        assert!(consider_known(&mut account, "instagram", &settings, &["Old11111".into(), "New22222".into()]).is_none());
    }
}
