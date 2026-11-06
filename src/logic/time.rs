//! Civil-time parsing for the timestamps X puts on `<time datetime>`.

/// Milliseconds since the Unix epoch. `None` when `raw` is not a complete
/// ISO-8601 datetime with a `Z` or numeric offset.
pub fn parse_time_ms(raw: &str) -> Option<i64> {
    let s = raw.trim();
    let bytes = s.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || !matches!(bytes.get(10), Some(b'T' | b't'))
    {
        return None;
    }
    let year: i32 = s.get(0..4)?.parse().ok()?;
    let month: u32 = s.get(5..7)?.parse().ok()?;
    let day: u32 = s.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let rest = s.get(11..)?;
    let hour: u32 = rest.get(0..2)?.parse().ok()?;
    let minute: u32 = rest.get(3..5)?.parse().ok()?;
    let second: u32 = rest.get(6..8)?.parse().ok()?;
    if rest.as_bytes().get(2) != Some(&b':')
        || rest.as_bytes().get(5) != Some(&b':')
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut idx = 8;
    let rest_bytes = rest.as_bytes();
    if rest_bytes.get(idx) == Some(&b'.') {
        idx += 1;
        let frac_start = idx;
        while idx < rest_bytes.len() && rest_bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx == frac_start {
            return None;
        }
    }
    let offset = parse_offset(rest.get(idx..)?)?;
    let days = days_from_civil(year, month, day)?;
    let local_ms = days * 86_400_000
        + i64::from(hour) * 3_600_000
        + i64::from(minute) * 60_000
        + i64::from(second) * 1000;
    Some(local_ms + offset)
}

fn parse_offset(raw: &str) -> Option<i64> {
    if raw == "Z" || raw == "z" {
        return Some(0);
    }
    let ahead = if let Some(rest) = raw.strip_prefix('+') {
        Some((true, rest))
    } else if let Some(rest) = raw.strip_prefix('-') {
        Some((false, rest))
    } else {
        None
    }?;
    let (ahead, body) = ahead;
    let (hour, minute) = if body.len() >= 5 && body.as_bytes().get(2) == Some(&b':') {
        (
            body.get(0..2)?.parse::<i64>().ok()?,
            body.get(3..5)?.parse::<i64>().ok()?,
        )
    } else if body.len() == 4 {
        (body.get(0..2)?.parse().ok()?, body.get(2..4)?.parse().ok()?)
    } else if body.len() == 2 {
        (body.parse().ok()?, 0)
    } else {
        return None;
    };
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
        return None;
    }
    let delta = (hour * 3_600 + minute * 60) * 1000;
    Some(if ahead { -delta } else { delta })
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// Howard Hinnant's civil-from-days inverse, days since 1970-01-01.
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let year = if month <= 2 { year - 1 } else { year } as i64;
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = (year - era * 400) as u64;
    let month_prime = if month > 2 { month - 3 } else { month + 9 } as u64;
    let day_of_year = (153 * month_prime + 2) / 5 + u64::from(day) - 1;
    let day_of_era = yoe * 365 + yoe / 4 - yoe / 100 + day_of_year;
    Some(era * 146_097 + day_of_era as i64 - 719_468)
}

#[cfg(test)]
mod tests {
    use super::parse_time_ms;

    #[test]
    fn unix_epoch_and_a_known_instant() {
        assert_eq!(parse_time_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_time_ms("2000-01-01T00:00:00Z"), Some(946_684_800_000));
    }

    #[test]
    fn offsets_and_fractions_collapse_to_utc() {
        let zulu = parse_time_ms("2026-03-01T12:30:00Z");
        assert_eq!(parse_time_ms("2026-03-01T20:30:00+08:00"), zulu);
        assert_eq!(
            parse_time_ms("2026-03-01T12:30:00.250Z"),
            zulu.map(|ms| ms + 0)
        );
        assert_eq!(parse_time_ms("2026-03-01T07:30:00-05:00"), zulu);
    }

    #[test]
    fn rejects_incomplete_or_impossible_dates() {
        assert_eq!(parse_time_ms("2026-03-01"), None);
        assert_eq!(parse_time_ms("2026-02-31T00:00:00Z"), None);
        assert_eq!(parse_time_ms("2026-03-01T12:30:00"), None);
        assert_eq!(parse_time_ms("not a time"), None);
    }
}
