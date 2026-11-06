//! A Chinese morning script from posts starred yesterday.
//!
//! The chat call uses the same OpenAI-compatible settings as translation.
//! No gateway, key, or model name is built in here. Yesterday is a local
//! calendar range the screen computes; this module only sees the two timestamps.

use serde_json::Value;

use super::model::{Briefing, Profile, Star};
use super::translate::{self, DEFAULT_MODEL};

pub const QUIET_EMPTY: &str = "昨天没有星标，今天不写早报。";
pub const QUIET_SETUP: &str = "还没有配置翻译接口，早报先不生成。";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub briefing: Briefing,
    /// When false, nothing was saved and the caller should not mark the day done.
    pub write: bool,
    /// When set, call the chat API with this body before saving `script`.
    pub request: Option<Value>,
}

pub fn stars_between<'a>(stars: &'a [Star], from_ms: i64, until_ms: i64) -> Vec<&'a Star> {
    let mut picked: Vec<&Star> = stars
        .iter()
        .filter(|star| star.starred_ms >= from_ms && star.starred_ms < until_ms && !star.body.trim().is_empty())
        .collect();
    picked.sort_by_key(|star| star.starred_ms);
    picked.truncate(12);
    picked
}

pub fn prompt(stars: &[&Star]) -> String {
    let mut text = String::from(
        "你是早报主播。下面是用户昨天星标的帖子，每条有平台、作者和原文。请读懂每条。如果某条需要一点你已经知道的背景，用一两句补上，不要编造链接或数字。然后写成一篇连贯的中文口播稿，适合听，不要条目清单，不要前言，不要说你是模型。控制在一到两分钟。\n",
    );
    for (index, star) in stars.iter().enumerate() {
        let platform = match star.platform.as_str() {
            "weibo" => "微博",
            "threads" => "Threads",
            "instagram" => "Instagram",
            "rss" => "RSS",
            _ => "X",
        };
        let body = star.body.chars().take(400).collect::<String>();
        let title = star.title.trim();
        text.push_str(&format!("\n{}. [{}] {}\n", index + 1, platform, star.author));
        if !title.is_empty() {
            text.push_str(title);
            text.push('\n');
        }
        text.push_str(&body);
        if !star.url.is_empty() {
            text.push_str("\n");
            text.push_str(&star.url);
        }
        text.push('\n');
    }
    text
}

pub fn request(model: &str, stars: &[&Star]) -> Value {
    let model = translate::canon_model(model).unwrap_or_else(|_| DEFAULT_MODEL.into());
    serde_json::json!({
        "model": model,
        "temperature": 0.4,
        "messages": [
            { "role": "system", "content": "你写中文口播稿。只输出可以念出来的正文。" },
            { "role": "user", "content": prompt(stars) }
        ]
    })
}

/// Decides whether a chat call is needed. `today` is `YYYY-MM-DD` in the device's zone.
pub fn prepare(profile: &Profile, from_ms: i64, until_ms: i64, today: &str, force: bool) -> Prepared {
    let today = today.trim();
    if !force && !profile.settings.briefing_auto {
        return Prepared {
            briefing: profile.briefing.clone(),
            write: false,
            request: None,
        };
    }
    if !force && profile.briefing.day == today && !today.is_empty() {
        return Prepared {
            briefing: profile.briefing.clone(),
            write: false,
            request: None,
        };
    }
    let picked = stars_between(&profile.stars, from_ms, until_ms);
    if picked.is_empty() {
        return Prepared {
            briefing: Briefing {
                day: today.to_string(),
                script: String::new(),
                note: QUIET_EMPTY.into(),
                count: 0,
            },
            write: true,
            request: None,
        };
    }
    if profile.settings.translate_url.trim().is_empty() || profile.settings.translate_token.trim().is_empty() {
        return Prepared {
            briefing: profile.briefing.clone(),
            write: false,
            request: None,
        };
    }
    Prepared {
        briefing: Briefing {
            day: today.to_string(),
            script: String::new(),
            note: String::new(),
            count: picked.len() as u32,
        },
        write: true,
        request: Some(request(&profile.settings.translate_model, &picked)),
    }
}

pub fn finish(mut prepared: Briefing, raw: &str) -> Result<Briefing, String> {
    let script = translate::read_translation(raw)?;
    prepared.script = script.chars().take(4000).collect();
    prepared.note.clear();
    Ok(prepared)
}

pub fn setup_note() -> &'static str {
    QUIET_SETUP
}

#[cfg(test)]
mod tests {
    use super::{prepare, prompt, stars_between, QUIET_EMPTY};
    use crate::logic::model::{Briefing, Card, Post, Profile, Star};
    use std::sync::Arc;

    fn star(id: &str, at: i64, body: &str) -> Star {
        Star {
            id: id.into(),
            platform: "rss".into(),
            handle: "https://example.com/feed".into(),
            author: "台账".into(),
            title: "外れたルール".into(),
            body: body.into(),
            url: "https://example.com/a".into(),
            starred_ms: at,
        }
    }

    #[test]
    fn yesterday_stars_become_one_spoken_script_request() {
        let mut profile = Profile::default();
        profile.stars.push(star("old", 1_000, "更早"));
        profile.stars.push(star("y", 5_000, "昨天星标的原文"));
        profile.settings.translate_url = "https://api.openai.com/v1".into();
        profile.settings.translate_token = "sk-test".into();
        let picked = stars_between(&profile.stars, 4_000, 9_000);
        assert_eq!(picked.len(), 1);
        let spoken = prompt(&picked);
        assert!(spoken.contains("口播"));
        assert!(spoken.contains("昨天星标的原文"));
        assert!(!spoken.contains("更早"));
        let prepared = prepare(&profile, 4_000, 9_000, "2026-11-06", false);
        assert!(prepared.write && prepared.request.is_some());
        let again = prepare(&profile, 4_000, 9_000, "2026-11-06", false);
        profile.briefing = Briefing {
            day: "2026-11-06".into(),
            script: "已有稿".into(),
            note: String::new(),
            count: 1,
        };
        let cached = prepare(&profile, 4_000, 9_000, "2026-11-06", false);
        assert!(!cached.write);
        assert_eq!(cached.briefing.script, "已有稿");
        let _ = again;
    }

    #[test]
    fn a_star_is_still_there_after_reload() {
        use crate::logic::model::{is_starred, normalize_json, toggle_star};
        let mut profile = Profile::default();
        profile.rss.cards.push(Card {
            handle: "https://example.com/feed".into(),
            posts: Arc::new(vec![Post {
                id: "p1".into(),
                text: "原文".into(),
                link_title: "题".into(),
                ..Post::default()
            }]),
            ..Card::default()
        });
        assert!(toggle_star(&mut profile, "rss", "p1").unwrap());
        assert!(is_starred(&profile, "rss", "p1"));
        let loaded = normalize_json(&serde_json::to_string(&profile).unwrap()).unwrap();
        assert_eq!(loaded.stars[0].body, "原文");
        assert!(is_starred(&loaded, "rss", "p1"));
        assert!(!toggle_star(&mut profile, "rss", "p1").unwrap());
        assert!(profile.stars.is_empty());
    }

    #[test]
    fn a_device_can_turn_auto_off_without_blocking_a_manual_run() {
        let mut profile = Profile::default();
        assert!(profile.settings.briefing_auto);
        profile.settings.briefing_auto = false;
        profile.settings.translate_url = "https://api.openai.com/v1".into();
        profile.settings.translate_token = "sk-test".into();
        profile.stars.push(star("y", 5_000, "昨天星标的原文"));
        let skipped = prepare(&profile, 4_000, 9_000, "2026-11-06", false);
        assert!(!skipped.write);
        assert!(skipped.request.is_none());
        let forced = prepare(&profile, 4_000, 9_000, "2026-11-06", true);
        assert!(forced.request.is_some());
        let mut saved = serde_json::to_value(&profile).unwrap();
        saved["settings"].as_object_mut().unwrap().remove("briefingAuto");
        let loaded: Profile = serde_json::from_value(saved).unwrap();
        assert!(loaded.settings.briefing_auto);
    }

    #[test]
    fn an_empty_yesterday_stays_quiet() {
        let mut profile = Profile::default();
        profile.rss.cards.push(Card {
            handle: "https://example.com/feed".into(),
            posts: Arc::new(vec![Post {
                id: "p".into(),
                text: "今天才写".into(),
                ..Post::default()
            }]),
            ..Card::default()
        });
        let prepared = prepare(&profile, 4_000, 9_000, "2026-11-06", false);
        assert!(prepared.write && prepared.request.is_none());
        assert_eq!(prepared.briefing.note, QUIET_EMPTY);
        assert!(prepared.briefing.script.is_empty());
        profile.settings.translate_url.clear();
        profile.stars.push(star("y", 5_000, "有星标但没接口"));
        let skipped = prepare(&profile, 4_000, 9_000, "2026-11-07", false);
        assert!(!skipped.write);
        assert!(skipped.request.is_none());
    }
}
