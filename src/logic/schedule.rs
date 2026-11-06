//! Which cards may open a web view, and which shell the window should use.
//!
//! Every platform is refreshed in the background, not only the one on screen.
//! At most [`model::MAX_SCRAPERS`] pages are open. The slots go to the due
//! cards attempted least recently, so one empty account cannot hold both.
//! Desktop windows use the square grid. A phone, and any window narrower
//! than a two-column grid, uses one column. iPad widths fall back to that
//! column in split view.

use serde::Serialize;

use super::model::{self, Card, Profile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchPlan {
    pub jobs: Vec<FetchJob>,
    pub max: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FetchJob {
    pub platform: String,
    pub index: u32,
}

/// At most [`model::MAX_SCRAPERS`] due accounts on any platform. Busy keys
/// are `platform:index`. `from` and `until` are the rows on screen; fetches
/// are not limited to them. Due cards are the ones whose last successful
/// fetch is older than the refresh interval, and whose last attempt (including
/// an empty one) is older than the failure backoff. The least recently
/// attempted of those is picked first.
pub fn next_fetches(
    profile: &Profile,
    now_ms: i64,
    busy: &[String],
    from: u32,
    until: u32,
) -> FetchPlan {
    let _ = (from, until);
    let interval_ms = (profile.settings.refresh_secs.saturating_mul(1000)) as i64;
    let mut due = Vec::new();
    for (lane, platform) in ["x", "threads", "instagram", "weibo", "rss"].into_iter().enumerate() {
        let cards = match platform {
            "threads" => &profile.threads.cards,
            "instagram" => &profile.instagram.cards,
            "weibo" => &profile.weibo.cards,
            "rss" => &profile.rss.cards,
            _ => &profile.x.cards,
        };
        // Feeds are polled at least every three minutes, even if the social
        // refresh is shorter, so a 30-second setting does not hammer them.
        let wait = if platform == "rss" { interval_ms.max(180_000) } else { interval_ms };
        for (index, card) in cards.iter().enumerate() {
            if !card_due(card, now_ms, wait) {
                continue;
            }
            let key = format!("{platform}:{index}");
            if busy.iter().any(|item| item == &key) {
                continue;
            }
            due.push((
                card.last_attempt_ms,
                lane,
                index as u32,
                FetchJob {
                    platform: platform.to_string(),
                    index: index as u32,
                },
            ));
        }
    }
    due.sort_by_key(|item| (item.0, item.1, item.2));
    let room = model::MAX_SCRAPERS.saturating_sub(busy.len());
    FetchPlan {
        jobs: due.into_iter().take(room).map(|item| item.3).collect(),
        max: model::MAX_SCRAPERS as u32,
    }
}

pub(crate) fn card_due(card: &Card, now_ms: i64, interval_ms: i64) -> bool {
    if card.handle.is_empty() {
        return false;
    }
    if card.fail_streak > 0 && card.last_attempt_ms != 0 {
        let since = now_ms.saturating_sub(card.last_attempt_ms as i64);
        if since < model::retry_backoff_ms(card.fail_streak, interval_ms as u64 / 1000) {
            return false;
        }
    }
    if card.last_fetch_ms == 0 {
        return true;
    }
    now_ms.saturating_sub(card.last_fetch_ms as i64) >= interval_ms
}

/// `"grid"` on a desktop-sized window, `"column"` on a phone or a narrow one.
pub fn shell_mode(os: &str, idiom: &str, width: f64) -> &'static str {
    if idiom == "phone" {
        return "column";
    }
    if os == "ios" && idiom != "mac" && width < 900.0 {
        return "column";
    }
    if width < 760.0 { "column" } else { "grid" }
}

#[cfg(test)]
mod tests {
    use super::{next_fetches, shell_mode, FetchJob};
    use crate::logic::model::{MAX_SCRAPERS, Profile};

    fn followed(handles: &[&str]) -> Profile {
        let mut profile = Profile::default();
        profile.focus = "x".into();
        for handle in handles {
            profile.x.cards.push(crate::logic::model::Card {
                handle: (*handle).into(),
                ..crate::logic::model::Card::default()
            });
        }
        profile
    }

    fn job(platform: &str, index: u32) -> FetchJob {
        FetchJob {
            platform: platform.into(),
            index,
        }
    }

    #[test]
    fn two_slots_walk_every_due_account_not_only_the_visible_rows() {
        let mut profile = followed(&["one", "two", "three", "four"]);
        let plan = next_fetches(&profile, 10_000, &[], 0, 1);
        assert_eq!(plan.max as usize, MAX_SCRAPERS);
        assert_eq!(plan.jobs, vec![job("x", 0), job("x", 1)]);
        profile.x.cards[0].last_fetch_ms = 10_000;
        profile.x.cards[1].last_fetch_ms = 10_000;
        let below = next_fetches(&profile, 10_000, &[], 0, 1);
        assert_eq!(below.jobs, vec![job("x", 2), job("x", 3)]);

        profile.x.cards[2].last_fetch_ms = 10_000;
        profile.x.cards[3].last_fetch_ms = 10_000;
        profile.settings.refresh_secs = 120;
        profile.x.cards[0].last_fetch_ms = 9_000;
        profile.x.cards[1].last_fetch_ms = 1_000;
        let plan = next_fetches(&profile, 10_000, &["x:1".into()], 0, 4);
        assert!(plan.jobs.is_empty() || plan.jobs.iter().all(|item| item.index != 1));
        let full = next_fetches(&profile, 10_000, &["x:0".into(), "x:1".into()], 0, 2);
        assert!(full.jobs.is_empty());
    }

    #[test]
    fn a_fresh_card_is_not_fetched_again_inside_the_interval() {
        let mut profile = followed(&["one"]);
        profile.settings.refresh_secs = 120;
        profile.x.cards[0].last_fetch_ms = 1_000;
        assert!(
            next_fetches(&profile, 1_000 + 119_000, &[], 0, 4)
                .jobs
                .is_empty()
        );
        assert_eq!(
            next_fetches(&profile, 1_000 + 120_000, &[], 0, 4).jobs,
            vec![job("x", 0)]
        );
    }

    #[test]
    fn every_platform_is_refreshed_even_when_another_tab_is_showing() {
        let mut profile = Profile::default();
        profile.focus = "x".into();
        profile.x.cards.push(crate::logic::model::Card {
            handle: "one".into(),
            ..crate::logic::model::Card::default()
        });
        profile.threads.cards.push(crate::logic::model::Card {
            handle: "two".into(),
            ..crate::logic::model::Card::default()
        });
        profile.instagram.cards.push(crate::logic::model::Card {
            handle: "three".into(),
            ..crate::logic::model::Card::default()
        });
        let plan = next_fetches(&profile, 10_000, &[], 0, 1);
        assert_eq!(plan.jobs, vec![job("x", 0), job("threads", 0)]);
    }

    #[test]
    fn an_empty_read_backs_off_and_does_not_starve_the_other_accounts() {
        let mut profile = Profile::default();
        profile.settings.refresh_secs = 120;
        profile.focus = "x".into();
        profile.instagram.cards.push(crate::logic::model::Card {
            handle: "aartisehwag".into(),
            last_attempt_ms: 10_000,
            fail_streak: 1,
            ..crate::logic::model::Card::default()
        });
        profile.instagram.cards.push(crate::logic::model::Card {
            handle: "zixin_1010".into(),
            ..crate::logic::model::Card::default()
        });
        profile.x.cards.push(crate::logic::model::Card {
            handle: "huggingpapers".into(),
            last_attempt_ms: 10_000,
            fail_streak: 1,
            ..crate::logic::model::Card::default()
        });
        for handle in ["vista8", "aigclink", "fxtrader"] {
            profile.x.cards.push(crate::logic::model::Card {
                handle: handle.into(),
                ..crate::logic::model::Card::default()
            });
        }
        let plan = next_fetches(&profile, 10_000, &[], 0, 0);
        assert_eq!(plan.jobs, vec![job("x", 1), job("x", 2)]);
        let still = next_fetches(&profile, 10_000 + 29_000, &[], 0, 0);
        let blocked = |item: &FetchJob| {
            (item.platform == "instagram" && item.index == 0)
                || (item.platform == "x" && item.index == 0)
        };
        assert!(still.jobs.iter().all(|item| !blocked(item)));
        for card in profile.x.cards.iter_mut().skip(1) {
            card.last_fetch_ms = 40_000;
            card.last_attempt_ms = 40_000;
        }
        profile.instagram.cards[1].last_fetch_ms = 40_000;
        profile.instagram.cards[1].last_attempt_ms = 40_000;
        let later = next_fetches(&profile, 40_000, &[], 0, 0);
        assert_eq!(later.jobs, vec![job("x", 0), job("instagram", 0)]);
    }

    #[test]
    fn phones_and_narrow_windows_use_one_column() {
        assert_eq!(shell_mode("ios", "phone", 1400.0), "column");
        assert_eq!(shell_mode("ios", "pad", 700.0), "column");
        assert_eq!(shell_mode("ios", "pad", 1000.0), "grid");
        assert_eq!(shell_mode("macos", "desktop", 1200.0), "grid");
        assert_eq!(shell_mode("linux", "desktop", 500.0), "column");
        assert_eq!(shell_mode("linux", "desktop", 1100.0), "grid");
    }
}
