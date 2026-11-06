//! The card's post list, after it has opened on the latest post.
//!
//! NSH's `Scroll` keeps its offset when the content grows. It does not follow
//! the end unless the app asks. This decides that ask: only when the reader
//! was already within 24px of the bottom and this event did not move them up.

/// The y offset to apply after a post-list update, or `-1` to leave it alone.
///
/// `prev_offset` / `old_content` are the position before the update.
/// `offset` / `new_content` are what the scroll view reported for this event.
pub fn next_offset(
    prev_offset: f64,
    offset: f64,
    viewport: f64,
    old_content: f64,
    new_content: f64,
) -> f64 {
    if ![prev_offset, offset, viewport, old_content, new_content]
        .iter()
        .all(|value| value.is_finite())
    {
        return -1.0;
    }
    if old_content <= 0.0 || new_content <= old_content + 1.0 {
        return -1.0;
    }
    let was_pinned = prev_offset + viewport >= old_content - 24.0;
    // Any upward move leaves the bottom. A few pixels still count: a refresh
    // must not pull the scrollbar back while the reader is dragging it.
    let moved_up = offset < prev_offset - 0.5;
    if was_pinned && !moved_up {
        return (new_content - viewport).max(0.0);
    }
    -1.0
}

#[cfg(test)]
mod tests {
    use super::next_offset;

    #[test]
    fn scrolling_up_is_not_pulled_back_when_the_list_grows() {
        let stayed = next_offset(400.0, 360.0, 280.0, 1200.0, 1500.0);
        assert_eq!(stayed, -1.0);
        let still = next_offset(400.0, 400.0, 280.0, 1200.0, 1500.0);
        assert_eq!(still, -1.0);
    }

    #[test]
    fn the_bottom_follows_only_while_it_stays_pinned() {
        let followed = next_offset(900.0, 900.0, 280.0, 1180.0, 1500.0);
        assert_eq!(followed, 1220.0);
        let leaving = next_offset(900.0, 860.0, 280.0, 1180.0, 1500.0);
        assert_eq!(leaving, -1.0);
        let nudge = next_offset(900.0, 898.0, 280.0, 1180.0, 1500.0);
        assert_eq!(nudge, -1.0);
    }
}
