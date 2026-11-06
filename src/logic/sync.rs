//! The subscription list that can move between devices.
//!
//! This is not the whole profile. Posts, read marks, cookies, and the sync
//! address stay on the device. Merging is a union: the same feed URL, folder
//! name, or account handle is kept once, and a local read mark is never
//! cleared because the remote list has no read state.

use super::model::{Card, Profile, MAX_CARDS};
use super::rss::canonical_feed_url;
use super::source::{self, valid_handle};
use super::text;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncFolder {
    #[serde(default)]
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncFeed {
    pub url: String,
    #[serde(default)]
    pub label: String,
    /// Remote folder id, matched to a name in `folders`. Empty means 未分类.
    #[serde(default)]
    pub folder: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncAccount {
    pub platform: String,
    pub handle: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncList {
    pub kind: String,
    pub version: u32,
    #[serde(default)]
    pub folders: Vec<SyncFolder>,
    #[serde(default)]
    pub rss: Vec<SyncFeed>,
    #[serde(default)]
    pub accounts: Vec<SyncAccount>,
}

const KIND: &str = "pulse-subscriptions";

pub fn export_list(profile: &Profile) -> SyncList {
    let folders = profile
        .folders
        .iter()
        .map(|folder| SyncFolder {
            id: folder.id.clone(),
            name: folder.name.clone(),
        })
        .collect();
    let rss = profile
        .rss
        .cards
        .iter()
        .filter(|card| !card.handle.is_empty())
        .map(|card| SyncFeed {
            url: card.handle.clone(),
            label: card.label.clone(),
            folder: card.folder.clone(),
        })
        .collect();
    let mut accounts = Vec::new();
    for platform in ["x", "threads", "instagram", "weibo"] {
        let board = match platform {
            "threads" => &profile.threads,
            "instagram" => &profile.instagram,
            "weibo" => &profile.weibo,
            _ => &profile.x,
        };
        for card in &board.cards {
            if card.handle.is_empty() {
                continue;
            }
            accounts.push(SyncAccount {
                platform: platform.to_string(),
                handle: card.handle.clone(),
                label: card.label.clone(),
            });
        }
    }
    SyncList {
        kind: KIND.into(),
        version: 1,
        folders,
        rss,
        accounts,
    }
}

pub fn export_json(profile: &Profile) -> String {
    serde_json::to_string_pretty(&export_list(profile)).unwrap_or_else(|_| "{}".into())
}

/// Unions `json` into `profile`. Returns how many folders, feeds, or accounts changed.
pub fn merge_json(profile: &mut Profile, json: &str) -> Result<u32, String> {
    let list: SyncList = serde_json::from_str(json).map_err(|_| "这份订阅列表无法读取。".to_string())?;
    if list.kind != KIND || list.version != 1 {
        return Err("这不是脉搏的订阅列表。".into());
    }
    let mut changed = 0u32;
    let mut folder_ids = std::collections::HashMap::<String, String>::new();
    for folder in &list.folders {
        let name = folder.name.trim();
        if name.is_empty() || name == "未分类" {
            continue;
        }
        let before = profile.folders.len();
        let id = match super::model::folder_for_name(profile, name) {
            Ok(id) => id,
            Err(_) => continue,
        };
        if profile.folders.len() != before {
            changed += 1;
        }
        if !folder.id.is_empty() {
            folder_ids.insert(folder.id.clone(), id);
        }
    }
    for feed in &list.rss {
        let Some(url) = canonical_feed_url(&feed.url) else {
            continue;
        };
        let folder = folder_ids.get(feed.folder.trim()).cloned().unwrap_or_default();
        let label = text::chars_at_most(feed.label.trim(), 80);
        if let Some(card) = profile.rss.cards.iter_mut().find(|card| card.handle.eq_ignore_ascii_case(&url)) {
            if card.label.trim().is_empty() && !label.is_empty() {
                card.label = label;
                changed += 1;
            }
            if card.folder.is_empty() && !folder.is_empty() {
                card.folder = folder;
                changed += 1;
            }
            continue;
        }
        if profile.rss.cards.len() >= MAX_CARDS {
            continue;
        }
        profile.rss.cards.push(Card {
            handle: url,
            label,
            folder,
            ..Card::default()
        });
        changed += 1;
    }
    for account in &list.accounts {
        let platform = source::canon_platform(&account.platform);
        if platform == "rss" {
            continue;
        }
        let handle = if platform == "weibo" {
            account.handle.trim().to_string()
        } else {
            account.handle.trim().trim_start_matches('@').to_ascii_lowercase()
        };
        if !valid_handle(platform, &handle) {
            continue;
        }
        let board = match platform {
            "threads" => &mut profile.threads,
            "instagram" => &mut profile.instagram,
            "weibo" => &mut profile.weibo,
            _ => &mut profile.x,
        };
        if board.cards.iter().any(|card| card.handle.eq_ignore_ascii_case(&handle)) {
            let label = text::chars_at_most(account.label.trim(), 80);
            if let Some(card) = board.cards.iter_mut().find(|card| card.handle.eq_ignore_ascii_case(&handle)) {
                if card.label.trim().is_empty() && !label.is_empty() {
                    card.label = label;
                    changed += 1;
                }
            }
            continue;
        }
        if board.cards.len() >= MAX_CARDS {
            continue;
        }
        board.cards.push(Card {
            handle,
            label: text::chars_at_most(account.label.trim(), 80),
            ..Card::default()
        });
        changed += 1;
    }
    Ok(changed)
}

pub fn new_token() -> String {
    let mut bytes = [0u8; 18];
    if let Ok(mut file) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = file.read(&mut bytes);
    } else {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let raw = nanos.to_le_bytes();
        bytes[..raw.len()].copy_from_slice(&raw);
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::model::{create_rss_folder, Card, Post};

    #[test]
    fn merging_twice_does_not_duplicate_and_keeps_local_reads() {
        let mut profile = Profile::default();
        let news = create_rss_folder(&mut profile, "新闻").unwrap();
        let mut card = Card {
            handle: "https://news.example/rss".into(),
            label: "本机".into(),
            folder: news.clone(),
            ..Card::default()
        };
        card.seen.push("r0000000000000001".into());
        std::sync::Arc::make_mut(&mut card.posts).push(Post {
            id: "r0000000000000001".into(),
            text: "already read".into(),
            time_ms: 4,
            ..Post::default()
        });
        profile.rss.cards.push(card);
        profile.x.cards.push(Card {
            handle: "kept".into(),
            label: "本地名".into(),
            ..Card::default()
        });
        let remote = r#"{
            "kind": "pulse-subscriptions",
            "version": 1,
            "folders": [{"id": "other", "name": "新闻"}, {"id": "blog", "name": "博客"}],
            "rss": [
                {"url": "https://news.example/rss", "label": "远端", "folder": "other"},
                {"url": "https://news.example/rss", "label": "重复", "folder": "blog"},
                {"url": "https://blog.example/feed", "label": "日志", "folder": "blog"}
            ],
            "accounts": [
                {"platform": "x", "handle": "Kept", "label": "远端名"},
                {"platform": "x", "handle": "@fresh", "label": "新的"},
                {"platform": "weibo", "handle": "1319518957", "label": "微博"}
            ]
        }"#;
        let first = merge_json(&mut profile, remote).unwrap();
        assert!(first >= 3);
        assert_eq!(profile.rss.cards.len(), 2);
        assert_eq!(profile.folders.len(), 2);
        let kept = profile.rss.cards.iter().find(|card| card.handle.contains("news.example")).unwrap();
        assert_eq!(kept.label, "本机");
        assert_eq!(kept.folder, news);
        assert_eq!(kept.seen, vec!["r0000000000000001".to_string()]);
        assert_eq!(kept.posts.len(), 1);
        let blog = profile.rss.cards.iter().find(|card| card.handle.contains("blog.example")).unwrap();
        assert_eq!(blog.label, "日志");
        assert_ne!(blog.folder, news);
        assert!(blog.seen.is_empty());
        assert_eq!(profile.x.cards.len(), 2);
        assert_eq!(profile.x.cards.iter().find(|card| card.handle == "kept").unwrap().label, "本地名");
        assert!(profile.x.cards.iter().any(|card| card.handle == "fresh"));
        assert_eq!(profile.weibo.cards[0].handle, "1319518957");
        assert_eq!(merge_json(&mut profile, remote).unwrap(), 0);
        assert!(merge_json(&mut profile, r#"{"version":1,"rss":[]}"#).is_err());
    }
}
