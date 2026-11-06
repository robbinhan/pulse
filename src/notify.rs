//! System notifications and the app icon badge.
//!
//! The decision of *what* to announce lives in [`crate::logic::notice`].
//! macOS posts through User Notifications (Dock badge, `osascript` if that
//! class is missing). iOS uses the same center for a local notification and
//! the icon badge. A background refresh can call [`deliver`] and [`set_badge`]
//! after it wakes; there is no APNs client here. Linux keeps the calls and
//! does nothing.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::logic::notice::Notice;

static OPEN: Mutex<VecDeque<(String, String)>> = Mutex::new(VecDeque::new());

/// A notification click asked the UI to show this account.
#[cfg_attr(not(any(target_os = "macos", target_os = "ios")), allow(dead_code))]
pub fn queue_open(platform: String, handle: String) {
    if let Ok(mut queue) = OPEN.lock() {
        queue.push_back((platform, handle));
    }
}

pub fn poll_open() -> Option<(String, String)> {
    OPEN.lock().ok()?.pop_front()
}

pub fn deliver(notice: &Notice) {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        crate::notify_mac::deliver(notice);
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        eprintln!(
            "[pulse] notify skipped (not apple) {} — {}",
            notice.title, notice.body
        );
    }
}

pub fn set_badge(count: u32) {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        crate::notify_mac::set_badge(count);
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let _ = count;
    }
}

pub fn ensure_auth() {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        crate::notify_mac::ensure_auth();
    }
}

/// One sample banner, for the settings button and `THEYSAY_NOTIFY_TEST=1`.
pub fn deliver_test() {
    deliver(&Notice {
        platform: "threads".into(),
        handle: String::new(),
        title: "脉搏 · 测试".into(),
        body: "这是一条测试通知。能看到它，说明系统通知已经接通。".into(),
        count: 1,
    });
}

pub fn maybe_test_from_env() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static ONCE: AtomicBool = AtomicBool::new(false);
    if std::env::var("THEYSAY_NOTIFY_TEST").ok().as_deref() == Some("1")
        && !ONCE.swap(true, Ordering::SeqCst)
    {
        eprintln!("[pulse] notify test from THEYSAY_NOTIFY_TEST");
        deliver_test();
    }
}
