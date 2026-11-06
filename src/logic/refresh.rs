//! Which RSS feeds a short iOS background wake should fetch.
//!
//! Social timelines need a web view, and a background refresh is only given a
//! few seconds. Those accounts stay on the foreground poll. This list is RSS
//! only, capped, and uses the same due rules as the on-screen scheduler.
//!
//! Remote push after the process is killed needs APNs (an Apple Developer
//! account, a push certificate, and a server). This module does not talk to
//! one. Background App Refresh is best-effort; iOS may never run it.

use super::model::Profile;
use super::schedule;

/// `BGTaskScheduler` identifier. It has to start with the bundle id.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub const TASK_ID: &str = "dev.pulse.app.refresh";

/// A wake is short. Leave the rest of the list for the next one.
pub const MAX_FEEDS: usize = 3;

/// iOS will not honor a sooner `BGAppRefreshTaskRequest`.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub const EARLIEST_SECS: f64 = 15.0 * 60.0;

/// Due RSS cards, least recently attempted first, at most [`MAX_FEEDS`].
pub fn due_rss(profile: &Profile, now_ms: i64) -> Vec<usize> {
    let interval_ms = (profile.settings.refresh_secs.saturating_mul(1000)) as i64;
    let wait = interval_ms.max(180_000);
    let mut due: Vec<(u64, usize)> = profile
        .rss
        .cards
        .iter()
        .enumerate()
        .filter(|(_, card)| schedule::card_due(card, now_ms, wait))
        .map(|(index, card)| (card.last_attempt_ms, index))
        .collect();
    due.sort_by_key(|item| (item.0, item.1));
    due.into_iter().take(MAX_FEEDS).map(|item| item.1).collect()
}

#[cfg(test)]
mod tests {
    use super::{due_rss, MAX_FEEDS, TASK_ID};
    use crate::logic::model::{Card, Profile};

    fn feed(handle: &str, fetched: u64, attempted: u64) -> Card {
        Card {
            handle: handle.into(),
            last_fetch_ms: fetched,
            last_attempt_ms: attempted,
            ..Card::default()
        }
    }

    #[test]
    fn background_task_id_matches_the_bundle() {
        assert_eq!(TASK_ID, "dev.pulse.app.refresh");
    }

    #[test]
    fn only_due_rss_feeds_and_only_three() {
        let mut profile = Profile::default();
        let now = 1_000_000_i64;
        profile.rss.cards.push(feed("fresh.example/rss", now as u64, now as u64));
        profile.rss.cards.push(feed("old-a.example/rss", 0, 10));
        profile.rss.cards.push(feed("old-b.example/rss", 0, 20));
        profile.rss.cards.push(feed("old-c.example/rss", 0, 30));
        profile.rss.cards.push(feed("old-d.example/rss", 0, 40));
        profile.x.cards.push(feed("someone", 0, 1));
        let indexes = due_rss(&profile, now);
        assert_eq!(indexes.len(), MAX_FEEDS);
        assert_eq!(indexes, vec![1, 2, 3]);
    }
}
