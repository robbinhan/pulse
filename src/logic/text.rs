//! Cashtags, cheap link labels, and the preview cut used in bubbles.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub kind: &'static str,
    pub text: String,
    pub symbol: String,
    pub tone: u8,
}

impl Segment {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            kind: "text",
            text: text.into(),
            symbol: String::new(),
            tone: 0,
        }
    }

    pub fn ticker(text: impl Into<String>, symbol: impl Into<String>) -> Self {
        let symbol = symbol.into();
        let tone = tone(&symbol);
        Self {
            kind: "ticker",
            text: text.into(),
            symbol,
            tone,
        }
    }
}

/// Stable badge tone in `1..=5` so `$NVDA` keeps its color.
pub fn tone(symbol: &str) -> u8 {
    let mut hash: u32 = 2_166_136_261;
    for byte in symbol.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(1_677_7619);
    }
    (hash % 5) as u8 + 1
}

/// Splits `text` into plain runs and `$TICKER` badges.
///
/// A ticker is `$` plus 1–6 letters, or that plus `.` and one letter
/// (`$BRK.B`). Amounts (`$12`) and letters glued to the dollar (`a$NVDA`)
/// stay plain text.
pub fn segment(text: &str) -> Vec<Segment> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '$' && (index == 0 || !is_symbol_char(chars[index - 1])) {
            if let Some((raw, symbol, next)) = read_ticker(&chars, index) {
                if !plain.is_empty() {
                    out.push(Segment::text(std::mem::take(&mut plain)));
                }
                out.push(Segment::ticker(raw, symbol));
                index = next;
                continue;
            }
        }
        plain.push(chars[index]);
        index += 1;
    }
    if !plain.is_empty() {
        out.push(Segment::text(plain));
    }
    if out.is_empty() {
        out.push(Segment::text(""));
    }
    out
}

fn is_symbol_char(char: char) -> bool {
    char.is_ascii_alphanumeric() || char == '_'
}

fn read_ticker(chars: &[char], dollar: usize) -> Option<(String, String, usize)> {
    let mut index = dollar + 1;
    let start = index;
    while index < chars.len() && chars[index].is_ascii_alphabetic() && index - start < 6 {
        index += 1;
    }
    if index == start {
        return None;
    }
    if index < chars.len() && chars[index].is_ascii_alphanumeric() {
        return None;
    }
    if index + 1 < chars.len()
        && chars[index] == '.'
        && chars[index + 1].is_ascii_alphabetic()
        && (index + 2 == chars.len() || !chars[index + 2].is_ascii_alphanumeric())
    {
        index += 2;
    }
    let raw: String = chars[dollar..index].iter().collect();
    let symbol: String = raw.trim_start_matches('$').to_ascii_uppercase();
    if symbol.is_empty() || symbol.len() > 8 {
        return None;
    }
    Some((raw, symbol, index))
}

/// Cuts a segmented line to `max` characters without slicing a badge in half.
pub fn truncate_segments(segments: Vec<Segment>, max: usize) -> Vec<Segment> {
    if max == 0 {
        return vec![Segment::text("…")];
    }
    let mut used = 0;
    let mut out = Vec::new();
    for segment in segments {
        let len = segment.text.chars().count();
        if used + len <= max {
            used += len;
            out.push(segment);
            continue;
        }
        let keep = max - used;
        if segment.kind == "text" && keep > 0 {
            let text: String = segment.text.chars().take(keep).collect();
            out.push(Segment::text(text));
        }
        out.push(Segment::text("…"));
        return out;
    }
    out
}

pub fn preview_segments(text: &str, max: usize) -> Vec<Segment> {
    truncate_segments(segment(text), max)
}

pub fn fold_space(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Hostname without `www.`, used when the page did not give a card title.
pub fn host_label(url: &str) -> String {
    let rest = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    host.trim_start_matches("www.").to_string()
}

pub fn first_https(text: &str) -> Option<String> {
    let start = text.find("https://")?;
    let rest = &text[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || matches!(c, ')' | '）' | '，' | '。' | '>' | '"'))
        .unwrap_or(rest.len());
    let url =
        rest[..end].trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | '!' | '?' | '…'));
    if url.len() > "https://a.co".len() {
        Some(url.to_string())
    } else {
        None
    }
}

pub fn chars_at_most(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        text.to_string()
    } else {
        text.chars().take(max).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{first_https, host_label, preview_segments, segment, tone};

    #[test]
    fn cashtags_become_badges_and_amounts_do_not() {
        let parts = segment("Watch $NVDA and $btc. Cash is $12 today.");
        let tickers: Vec<_> = parts
            .iter()
            .filter(|part| part.kind == "ticker")
            .map(|part| part.symbol.as_str())
            .collect();
        assert_eq!(tickers, ["NVDA", "BTC"]);
        assert!(
            parts
                .iter()
                .any(|part| part.kind == "text" && part.text.contains("$12"))
        );
        assert_eq!(
            segment("($NVDA)")
                .iter()
                .find(|part| part.kind == "ticker")
                .map(|part| part.symbol.as_str()),
            Some("NVDA")
        );
        assert!(segment("end$NVDA").iter().all(|part| part.kind == "text"));
        assert!(
            segment("$ABCDEFG no")
                .iter()
                .all(|part| part.kind == "text")
        );
        assert_eq!(segment("$BRK.B rallied")[0].symbol, "BRK.B");
        assert_eq!(segment("$NVDA.")[0].symbol, "NVDA");
    }

    #[test]
    fn tones_stay_in_range_and_stable() {
        assert_eq!(tone("NVDA"), tone("NVDA"));
        assert!((1..=5).contains(&tone("NVDA")));
        assert!((1..=5).contains(&tone("BTC")));
    }

    #[test]
    fn preview_does_not_slice_a_badge() {
        let text = format!("{} $NVDA", "字".repeat(8));
        let parts = preview_segments(&text, 8);
        assert!(parts.iter().all(|part| part.kind != "ticker"));
        assert_eq!(parts.last().map(|part| part.text.as_str()), Some("…"));
    }

    #[test]
    fn cheap_link_labels() {
        assert_eq!(host_label("https://www.example.com/notes/1"), "example.com");
        assert_eq!(
            first_https("see https://example.com/a. thanks"),
            Some("https://example.com/a".into())
        );
    }
}
