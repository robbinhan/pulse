//! iCloud container for the subscription list.
//!
//! `URLForUbiquityContainerIdentifier` is nil when the signed app has no
//! iCloud entitlement. Sideloaded Pulse packages are in that case. This
//! module reports that instead of writing a local file and calling it iCloud.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcloudStatus {
    pub available: bool,
    pub message: String,
}

const FILE_NAME: &str = "pulse-subscriptions.json";

pub fn status() -> IcloudStatus {
    match container_dir() {
        Some(_) => IcloudStatus {
            available: true,
            message: "可用".into(),
        },
        None => IcloudStatus {
            available: false,
            message: unavailable_message(),
        },
    }
}

pub fn read_list() -> Result<String, String> {
    let path = list_path()?;
    match std::fs::read_to_string(&path) {
        Ok(text) if !text.trim().is_empty() => Ok(text),
        Ok(_) => Err("iCloud 里还没有订阅列表。".into()),
        Err(_) => Err("iCloud 里还没有订阅列表。".into()),
    }
}

pub fn write_list(json: &str) -> Result<(), String> {
    let path = list_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "iCloud 文件夹无法创建。".to_string())?;
    }
    std::fs::write(&path, json).map_err(|_| "没有写进 iCloud。".to_string())?;
    Ok(())
}

fn list_path() -> Result<PathBuf, String> {
    let Some(dir) = container_dir() else {
        return Err(unavailable_message());
    };
    Ok(dir.join(FILE_NAME))
}

fn unavailable_message() -> String {
    "当前这个签名包没有 iCloud 容器。旁加载的 Mac 和 iPhone 包用不了 iCloud，请改选自建服务器。".into()
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn container_dir() -> Option<PathBuf> {
    apple::ubiquity_documents()
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn container_dir() -> Option<PathBuf> {
    None
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple {
    use std::ffi::{c_char, c_void, CStr};
    use std::path::PathBuf;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send();
    }

    #[cfg(target_os = "macos")]
    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {}

    #[cfg(target_os = "ios")]
    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {}

    fn sel(name: &str) -> *mut c_void {
        let c = std::ffi::CString::new(name).unwrap_or_default();
        unsafe { sel_registerName(c.as_ptr()) }
    }

    fn class(name: &str) -> *mut c_void {
        let c = std::ffi::CString::new(name).unwrap_or_default();
        unsafe { objc_getClass(c.as_ptr()) }
    }

    fn msg0(obj: *mut c_void, sel: *mut c_void) -> *mut c_void {
        type F = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
        let f: F = unsafe { std::mem::transmute(objc_msg_send as *const ()) };
        unsafe { f(obj, sel) }
    }

    fn msg_ptr(obj: *mut c_void, sel: *mut c_void, arg: *mut c_void) -> *mut c_void {
        type F = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
        let f: F = unsafe { std::mem::transmute(objc_msg_send as *const ()) };
        unsafe { f(obj, sel, arg) }
    }

    pub fn ubiquity_documents() -> Option<PathBuf> {
        unsafe {
            let fm = msg0(class("NSFileManager"), sel("defaultManager"));
            if fm.is_null() {
                return None;
            }
            let url = msg_ptr(fm, sel("URLForUbiquityContainerIdentifier:"), std::ptr::null_mut());
            if url.is_null() {
                return None;
            }
            let path = msg0(url, sel("path"));
            if path.is_null() {
                return None;
            }
            let bytes = msg0(path, sel("UTF8String")) as *const c_char;
            if bytes.is_null() {
                return None;
            }
            let text = CStr::from_ptr(bytes).to_string_lossy();
            if text.is_empty() {
                return None;
            }
            Some(PathBuf::from(text.as_ref()).join("Documents"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::status;

    #[test]
    fn this_host_does_not_pretend_icloud_works() {
        let report = status();
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            assert!(!report.message.is_empty());
        } else {
            assert!(!report.available);
            assert!(report.message.contains("iCloud"));
        }
    }
}
