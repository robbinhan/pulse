//! First launch after the bundle id changed from `dev.theysay.native` to
//! `dev.pulse.app`. NSH keeps `storage.json` under the id, and WKWebView's
//! default data store follows the same id. Nothing in NSH 0.3.0 names a
//! website data store, so the old directories are copied once, before the
//! first page opens.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const BUNDLE_ID: &str = "dev.pulse.app";
pub const LEGACY_BUNDLE_ID: &str = "dev.theysay.native";
pub const SUPPORT_DIR: &str = "Pulse";
pub const LEGACY_SUPPORT_DIR: &str = "TheySay";

/// Keys the screen still writes. Older builds used the same names.
pub const PROFILE_KEY: &str = "theysay.profile.v1";
pub const SESSION_KEY: &str = "theysay.session.v1";
/// Read when a renamed copy stored the dashboard under the new names.
pub const PROFILE_KEY_ALT: &str = "pulse.profile.v1";
pub const SESSION_KEY_ALT: &str = "pulse.session.v1";

const PROFILE_KEYS: [&str; 2] = [PROFILE_KEY, PROFILE_KEY_ALT];
const SESSION_KEYS: [&str; 2] = [SESSION_KEY, SESSION_KEY_ALT];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    pub profile: bool,
    pub storage: bool,
    pub webkit: bool,
}

impl Report {
    pub fn copied(self) -> bool {
        self.profile || self.storage || self.webkit
    }
}

pub fn migrate(home: &Path) -> Report {
    Report {
        profile: copy_profile(home),
        storage: copy_storage(home),
        webkit: copy_webkit(home),
    }
}

pub fn profile_text(home: &Path) -> Option<String> {
    value_from(&storage_file(home, BUNDLE_ID), &PROFILE_KEYS)
        .or_else(|| value_from(&storage_file(home, LEGACY_BUNDLE_ID), &PROFILE_KEYS))
}

pub fn session_text(home: &Path) -> Option<String> {
    value_from(&storage_file(home, BUNDLE_ID), &SESSION_KEYS)
        .or_else(|| value_from(&storage_file(home, LEGACY_BUNDLE_ID), &SESSION_KEYS))
}

fn copy_profile(home: &Path) -> bool {
    let dest = support_dir(home, SUPPORT_DIR).join("profile.json");
    if file_filled(&dest) {
        return false;
    }
    let src = support_dir(home, LEGACY_SUPPORT_DIR).join("profile.json");
    if !file_filled(&src) {
        return false;
    }
    let Some(parent) = dest.parent() else {
        return false;
    };
    if fs::create_dir_all(parent).is_err() {
        return false;
    }
    fs::copy(&src, &dest).is_ok()
}

fn copy_storage(home: &Path) -> bool {
    let dest = storage_file(home, BUNDLE_ID);
    let src = storage_file(home, LEGACY_BUNDLE_ID);
    let Some(old) = read_map(&src) else {
        return false;
    };
    if !map_has_state(&old) {
        return false;
    }
    let mut current = read_map(&dest).unwrap_or_default();
    if map_has_state(&current) {
        return false;
    }
    if current.is_empty() {
        current = old;
    } else {
        for key in PROFILE_KEYS.into_iter().chain(SESSION_KEYS) {
            if filled(current.get(key).map(String::as_str)) {
                continue;
            }
            if let Some(value) = old.get(key) {
                if !value.trim().is_empty() {
                    current.insert(key.to_string(), value.clone());
                }
            }
        }
    }
    write_map(&dest, &current)
}

fn copy_webkit(home: &Path) -> bool {
    // Website data only. The `*.binarycookies` file is not copied: WebKit
    // replaces that file when the first page opens, which wiped a previous
    // copy. A directory that already exists is left untouched, including an
    // empty one the data store may have created.
    let pairs = [
        (
            home.join("Library/WebKit").join(LEGACY_BUNDLE_ID),
            home.join("Library/WebKit").join(BUNDLE_ID),
        ),
        (
            home.join("Library/Caches").join(LEGACY_BUNDLE_ID),
            home.join("Library/Caches").join(BUNDLE_ID),
        ),
        (
            home.join("Library/HTTPStorages").join(LEGACY_BUNDLE_ID),
            home.join("Library/HTTPStorages").join(BUNDLE_ID),
        ),
    ];
    let mut copied = false;
    for (src, dest) in pairs {
        if !src.exists() || dest.exists() {
            continue;
        }
        if copy_tree(&src, &dest) {
            copied = true;
        }
    }
    copied
}

fn support_dir(home: &Path, name: &str) -> PathBuf {
    home.join("Library/Application Support").join(name)
}

fn storage_file(home: &Path, id: &str) -> PathBuf {
    support_dir(home, id).join("storage.json")
}

fn map_has_state(map: &BTreeMap<String, String>) -> bool {
    PROFILE_KEYS
        .into_iter()
        .chain(SESSION_KEYS)
        .any(|key| filled(map.get(key).map(String::as_str)))
}

fn filled(value: Option<&str>) -> bool {
    value.is_some_and(|text| !text.trim().is_empty())
}

fn file_filled(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .is_some_and(|text| !text.trim().is_empty())
}

fn read_map(path: &Path) -> Option<BTreeMap<String, String>> {
    let text = fs::read_to_string(path).ok()?;
    if text.trim().is_empty() {
        return Some(BTreeMap::new());
    }
    serde_json::from_str(&text).ok()
}

fn write_map(path: &Path, map: &BTreeMap<String, String>) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if fs::create_dir_all(parent).is_err() {
        return false;
    }
    let Ok(body) = serde_json::to_vec_pretty(map) else {
        return false;
    };
    fs::write(path, body).is_ok()
}

fn value_from(path: &Path, keys: &[&str]) -> Option<String> {
    let map = read_map(path)?;
    for key in keys {
        if let Some(value) = map.get(*key) {
            if !value.trim().is_empty() {
                return Some(value.clone());
            }
        }
    }
    None
}

fn copy_tree(src: &Path, dest: &Path) -> bool {
    if src.is_file() {
        let Some(parent) = dest.parent() else {
            return false;
        };
        if fs::create_dir_all(parent).is_err() {
            return false;
        }
        return fs::copy(src, dest).is_ok();
    }
    if !src.is_dir() {
        return false;
    }
    if fs::create_dir_all(dest).is_err() {
        return false;
    }
    let Ok(entries) = fs::read_dir(src) else {
        return false;
    };
    let mut ok = true;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !copy_tree(&entry.path(), &dest.join(name)) {
            ok = false;
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pulse-legacy-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn an_empty_pulse_store_takes_the_old_dashboard_and_cookies() {
        let root = home("copy");
        let profile = support_dir(&root, LEGACY_SUPPORT_DIR).join("profile.json");
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        fs::write(&profile, r#"{"focus":"x"}"#).unwrap();
        let storage = storage_file(&root, LEGACY_BUNDLE_ID);
        fs::create_dir_all(storage.parent().unwrap()).unwrap();
        fs::write(
            &storage,
            r#"{"theysay.profile.v1":"{\"focus\":\"threads\"}","theysay.session.v1":"{\"x\":\"in\"}"}"#,
        )
        .unwrap();
        let cookie = root
            .join("Library/WebKit")
            .join(LEGACY_BUNDLE_ID)
            .join("WebsiteData/cookies");
        fs::create_dir_all(cookie.parent().unwrap()).unwrap();
        fs::write(&cookie, "session").unwrap();

        let report = migrate(&root);
        assert!(report.profile && report.storage && report.webkit);
        assert_eq!(
            fs::read_to_string(support_dir(&root, SUPPORT_DIR).join("profile.json")).unwrap(),
            r#"{"focus":"x"}"#
        );
        assert_eq!(
            profile_text(&root).as_deref(),
            Some(r#"{"focus":"threads"}"#)
        );
        assert_eq!(session_text(&root).as_deref(), Some(r#"{"x":"in"}"#));
        let moved = root
            .join("Library/WebKit")
            .join(BUNDLE_ID)
            .join("WebsiteData/cookies");
        assert_eq!(fs::read_to_string(moved).unwrap(), "session");

        fs::write(support_dir(&root, SUPPORT_DIR).join("profile.json"), "kept").unwrap();
        let again = migrate(&root);
        assert!(!again.copied());
        assert_eq!(
            fs::read_to_string(support_dir(&root, SUPPORT_DIR).join("profile.json")).unwrap(),
            "kept"
        );
        let kept_cookie = root
            .join("Library/WebKit")
            .join(BUNDLE_ID)
            .join("WebsiteData/cookies");
        fs::write(&kept_cookie, "current").unwrap();
        fs::write(&cookie, "replaced-old").unwrap();
        assert!(!migrate(&root).webkit);
        assert_eq!(fs::read_to_string(&kept_cookie).unwrap(), "current");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_pulse_cookie_file_is_never_written_or_replaced() {
        let root = home("cookies");
        let old = root
            .join("Library/Cookies")
            .join(format!("{LEGACY_BUNDLE_ID}.binarycookies"));
        let current = root
            .join("Library/Cookies")
            .join(format!("{BUNDLE_ID}.binarycookies"));
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, "old-session").unwrap();
        fs::write(&current, "x-and-instagram").unwrap();
        assert!(!migrate(&root).webkit);
        assert_eq!(fs::read_to_string(&current).unwrap(), "x-and-instagram");
        fs::remove_file(&current).unwrap();
        assert!(!migrate(&root).webkit);
        assert!(!current.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_renamed_key_still_loads() {
        let root = home("alt");
        let storage = storage_file(&root, LEGACY_BUNDLE_ID);
        fs::create_dir_all(storage.parent().unwrap()).unwrap();
        fs::write(
            &storage,
            r#"{"pulse.profile.v1":"{\"focus\":\"weibo\"}","pulse.session.v1":"{\"weibo\":\"in\"}"}"#,
        )
        .unwrap();
        assert!(migrate(&root).storage);
        assert_eq!(profile_text(&root).as_deref(), Some(r#"{"focus":"weibo"}"#));
        assert_eq!(session_text(&root).as_deref(), Some(r#"{"weibo":"in"}"#));
        let _ = fs::remove_dir_all(&root);
    }
}
