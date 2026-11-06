//! The script injected into the hidden X profile web view.

const SCRIPT: &str = include_str!("../extract.js");

pub fn extract_script(token: &str) -> String {
    let literal = serde_json::to_string(token).unwrap_or_else(|_| "\"x\"".into());
    let dump = match std::env::var("THEYSAY_DUMP") {
        Ok(value) if value == "1" => "true",
        _ => "false",
    };
    SCRIPT
        .replace("__NSH_TOKEN__", &literal)
        .replace("__NSH_DUMP__", dump)
}

pub fn safe_token(token: &str) -> String {
    let cleaned: String = token
        .chars()
        .filter(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_' | ':' | '.'))
        .take(80)
        .collect();
    if cleaned.is_empty() {
        "x".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_script, safe_token};

    #[test]
    fn the_script_posts_to_nsh_and_does_not_touch_secrets() {
        let token = safe_token("card0:3");
        let script = extract_script(&token);
        assert!(script.contains("window.nsh.postMessage"));
        assert!(
            script.contains("article[data-testid=\"tweet\"]")
                || script.contains("data-testid=\\\"tweet\\\"")
                || script.contains("tweet")
        );
        assert!(script.contains(&format!("\"{token}\"")));
        assert!(!script.contains("__NSH_TOKEN__"));
        assert!(!script.contains("__NSH_DUMP__"));
        assert!(script.contains("var DUMP = false"));
        let lower = script.to_ascii_lowercase();
        assert!(!lower.contains("document.cookie"));
        assert!(!lower.contains("localstorage"));
        assert!(!lower.contains("password"));
        assert!(!lower.contains("eval("));
    }

    #[test]
    fn a_hostile_token_cannot_break_out_of_the_string() {
        let script = extract_script(&safe_token("\";alert(1);//"));
        assert!(script.contains("\"alert1\"") || !script.contains("alert(1)"));
        assert!(!script.contains("__NSH_TOKEN__"));
    }
}
