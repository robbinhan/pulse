//! Local notifications through `UNUserNotificationCenter`.
//!
//! On macOS the Dock badge is `NSDockTile`, and a missing or rejecting center
//! falls back to `osascript`. On iOS there is no AppKit and no `osascript`:
//! the icon badge is `setBadgeCount` (or `UIApplication` on older systems),
//! and a rejection is only logged. A click records the account. macOS also
//! activates the app; iOS already brings it forward.

use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::logic::notice::Notice;
use crate::notify;

#[link(name = "objc")]
#[allow(non_snake_case)]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_allocateClassPair(superclass: *mut c_void, name: *const c_char, extra: usize) -> *mut c_void;
    fn objc_registerClassPair(class: *mut c_void);
    fn class_addMethod(
        class: *mut c_void,
        name: *mut c_void,
        imp: *mut c_void,
        types: *const c_char,
    ) -> u8;
    static _NSConcreteStackBlock: c_void;
    fn _Block_copy(block: *const c_void) -> *mut c_void;
    #[link_name = "objc_msgSend"]
    fn objc_msg_send();
}

#[cfg(target_os = "macos")]
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}
#[cfg(target_os = "ios")]
#[link(name = "UIKit", kind = "framework")]
unsafe extern "C" {}
#[link(name = "UserNotifications", kind = "framework")]
unsafe extern "C" {}

#[repr(C)]
struct BlockDesc {
    reserved: usize,
    size: usize,
}

#[repr(C)]
struct VoidBlock {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: *const c_void,
    descriptor: *const BlockDesc,
}

const AUTH: usize = (1 << 2) | (1 << 1) | (1 << 0);
const PRESENT_BANNER: usize = (1 << 4) | (1 << 3) | (1 << 1);

pub fn ensure_auth() {
    static ONCE: AtomicBool = AtomicBool::new(false);
    if ONCE.swap(true, Ordering::SeqCst) {
        return;
    }
    if class("UNUserNotificationCenter").is_null() {
        #[cfg(target_os = "macos")]
        eprintln!("[pulse] notify path=osascript (UNUserNotificationCenter is absent)");
        #[cfg(target_os = "ios")]
        eprintln!("[pulse] notify skipped (UNUserNotificationCenter is absent)");
        return;
    }
    install_delegate();
    let center = msg0(class("UNUserNotificationCenter"), sel("currentNotificationCenter"));
    let block = heap_block(auth_invoke as *const c_void);
    msg_ptr2(
        center,
        sel("requestAuthorizationWithOptions:completionHandler:"),
        AUTH as *mut c_void,
        block,
    );
}

pub fn deliver(notice: &Notice) {
    ensure_auth();
    if class("UNUserNotificationCenter").is_null() || !post_un(notice) {
        fallback(notice);
    }
}

pub fn set_badge(count: u32) {
    #[cfg(target_os = "macos")]
    set_dock_badge(count);
    #[cfg(target_os = "ios")]
    set_icon_badge(count);
}

#[cfg(target_os = "macos")]
fn set_dock_badge(count: u32) {
    let app = msg0(class("NSApplication"), sel("sharedApplication"));
    if app.is_null() {
        return;
    }
    let tile = msg0(app, sel("dockTile"));
    if tile.is_null() {
        return;
    }
    if count == 0 {
        msg_ptr(tile, sel("setBadgeLabel:"), std::ptr::null_mut());
    } else {
        let label = ns_string(&count.min(999).to_string());
        msg_ptr(tile, sel("setBadgeLabel:"), label);
    }
}

/// Icon badge while the process is running. `setBadgeCount` is iOS 16;
/// `applicationIconBadgeNumber` covers an older deployment target.
#[cfg(target_os = "ios")]
fn set_icon_badge(count: u32) {
    let count = count.min(999) as usize as *mut c_void;
    let center = msg0(class("UNUserNotificationCenter"), sel("currentNotificationCenter"));
    let modern = sel("setBadgeCount:withCompletionHandler:");
    if msg_u8(center, sel("respondsToSelector:"), modern) != 0 {
        let _ = msg_ptr2(center, modern, count, std::ptr::null_mut());
        return;
    }
    let app = msg0(class("UIApplication"), sel("sharedApplication"));
    if !app.is_null() {
        msg_ptr(app, sel("setApplicationIconBadgeNumber:"), count);
    }
}

fn post_un(notice: &Notice) -> bool {
    let center = msg0(class("UNUserNotificationCenter"), sel("currentNotificationCenter"));
    let content = msg0(class("UNMutableNotificationContent"), sel("new"));
    if center.is_null() || content.is_null() {
        return false;
    }
    msg_ptr(content, sel("setTitle:"), ns_string(&notice.title));
    msg_ptr(content, sel("setBody:"), ns_string(&notice.body));
    msg_ptr(
        content,
        sel("setThreadIdentifier:"),
        ns_string(&format!("{}:{}", notice.platform, notice.handle)),
    );
    let info = msg0(class("NSMutableDictionary"), sel("dictionary"));
    msg_ptr2(
        info,
        sel("setObject:forKey:"),
        ns_string(&notice.platform),
        ns_string("platform"),
    );
    msg_ptr2(
        info,
        sel("setObject:forKey:"),
        ns_string(&notice.handle),
        ns_string("handle"),
    );
    msg_ptr(content, sel("setUserInfo:"), info);
    let ident = ns_string(&format!(
        "theysay-{}-{}",
        notice.platform,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let request = msg3(
        class("UNNotificationRequest"),
        sel("requestWithIdentifier:content:trigger:"),
        ident,
        content,
        std::ptr::null_mut(),
    );
    if request.is_null() {
        return false;
    }
    #[cfg(target_os = "macos")]
    remember_fallback(notice);
    let block = heap_block(added_invoke as *const c_void);
    msg_ptr2(center, sel("addNotificationRequest:withCompletionHandler:"), request, block);
    true
}

unsafe extern "C" fn auth_invoke(_block: *mut c_void, granted: u8, error: *mut c_void) {
    if granted == 0 || !error.is_null() {
        #[cfg(target_os = "macos")]
        eprintln!("[pulse] notify auth unavailable, later posts may use path=osascript");
        #[cfg(target_os = "ios")]
        eprintln!("[pulse] notify auth unavailable");
    } else {
        eprintln!("[pulse] notify auth granted path=user-notifications");
    }
}

unsafe extern "C" fn added_invoke(_block: *mut c_void, error: *mut c_void) {
    if error.is_null() {
        eprintln!("[pulse] notify path=user-notifications");
        #[cfg(target_os = "macos")]
        clear_fallback();
    } else {
        #[cfg(target_os = "macos")]
        {
            eprintln!("[pulse] notify path=osascript (user-notifications rejected the request)");
            if let Some(notice) = take_fallback() {
                osascript(&notice);
            }
        }
        #[cfg(target_os = "ios")]
        eprintln!("[pulse] notify path=user-notifications rejected");
    }
}

unsafe extern "C" fn will_present(
    _this: *mut c_void,
    _sel: *mut c_void,
    _center: *mut c_void,
    _note: *mut c_void,
    completion: *mut c_void,
) {
    unsafe { invoke_usize(completion, PRESENT_BANNER) }
}

unsafe extern "C" fn did_receive(
    _this: *mut c_void,
    _sel: *mut c_void,
    _center: *mut c_void,
    response: *mut c_void,
    completion: *mut c_void,
) {
    unsafe {
        if let Some((platform, handle)) = read_response(response) {
            eprintln!("[pulse] notify click platform={platform} handle={handle}");
            notify::queue_open(platform, handle);
            #[cfg(target_os = "macos")]
            activate_app();
        }
        invoke_void(completion);
    }
}

fn install_delegate() {
    static READY: AtomicBool = AtomicBool::new(false);
    if READY.swap(true, Ordering::SeqCst) {
        return;
    }
    let name = CString::new("TheySayNoteDelegate").unwrap();
    let subclass = unsafe { objc_allocateClassPair(class("NSObject"), name.as_ptr(), 0) };
    if subclass.is_null() {
        return;
    }
    let present = CString::new("v@:@@@?").unwrap();
    unsafe {
        class_addMethod(
            subclass,
            sel("userNotificationCenter:willPresentNotification:withCompletionHandler:"),
            will_present as *const () as *mut c_void,
            present.as_ptr(),
        );
        let receive = CString::new("v@:@@@?").unwrap();
        class_addMethod(
            subclass,
            sel("userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:"),
            did_receive as *const () as *mut c_void,
            receive.as_ptr(),
        );
        objc_registerClassPair(subclass);
    }
    let delegate = msg0(subclass, sel("new"));
    let center = msg0(class("UNUserNotificationCenter"), sel("currentNotificationCenter"));
    msg_ptr(center, sel("setDelegate:"), delegate);
}

unsafe fn read_response(response: *mut c_void) -> Option<(String, String)> {
    if response.is_null() {
        return None;
    }
    let notification = msg0(response, sel("notification"));
    let request = msg0(notification, sel("request"));
    let content = msg0(request, sel("content"));
    let info = msg0(content, sel("userInfo"));
    let platform = unsafe { dict(info, "platform") }?;
    let handle = unsafe { dict(info, "handle") }.unwrap_or_default();
    Some((platform, handle))
}

unsafe fn dict(info: *mut c_void, key: &str) -> Option<String> {
    if info.is_null() {
        return None;
    }
    let value = msg_ptr_ret(info, sel("objectForKey:"), ns_string(key));
    if value.is_null() {
        return None;
    }
    let bytes: *const c_char = msg0(value, sel("UTF8String")) as *const c_char;
    if bytes.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(bytes) }.to_string_lossy().into_owned())
}

#[cfg(target_os = "macos")]
unsafe fn activate_app() {
    let app = msg0(class("NSApplication"), sel("sharedApplication"));
    if app.is_null() {
        return;
    }
    msg_ptr(app, sel("activateIgnoringOtherApps:"), 1usize as *mut c_void);
}

#[cfg(target_os = "macos")]
fn remember_fallback(notice: &Notice) {
    if let Ok(mut slot) = FALLBACK.lock() {
        *slot = Some(notice.clone());
    }
}

#[cfg(target_os = "macos")]
fn take_fallback() -> Option<Notice> {
    FALLBACK.lock().ok()?.take()
}

#[cfg(target_os = "macos")]
fn clear_fallback() {
    if let Ok(mut slot) = FALLBACK.lock() {
        *slot = None;
    }
}

#[cfg(target_os = "macos")]
static FALLBACK: std::sync::Mutex<Option<Notice>> = std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
fn fallback(notice: &Notice) {
    osascript(notice);
}

#[cfg(target_os = "ios")]
fn fallback(notice: &Notice) {
    eprintln!(
        "[pulse] notify skipped (user notifications unavailable) {} — {}",
        notice.title, notice.body
    );
}

#[cfg(target_os = "macos")]
fn osascript(notice: &Notice) {
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        apple(&notice.body),
        apple(&notice.title)
    );
    match std::process::Command::new("osascript").arg("-e").arg(script).status() {
        Ok(status) if status.success() => eprintln!("[pulse] notify path=osascript"),
        Ok(status) => eprintln!("[pulse] notify path=osascript exit={status}"),
        Err(err) => eprintln!("[pulse] notify path=osascript failed {err}"),
    }
}

#[cfg(target_os = "macos")]
fn apple(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn class(name: &str) -> *mut c_void {
    let c = CString::new(name).unwrap();
    unsafe { objc_getClass(c.as_ptr()) }
}

fn sel(name: &str) -> *mut c_void {
    let c = CString::new(name).unwrap();
    unsafe { sel_registerName(c.as_ptr()) }
}

fn ns_string(text: &str) -> *mut c_void {
    let Ok(c) = CString::new(text) else {
        return msg0(class("NSString"), sel("string"));
    };
    msg_ptr_ret(class("NSString"), sel("stringWithUTF8String:"), c.as_ptr() as *mut c_void)
}

fn heap_block(invoke: *const c_void) -> *mut c_void {
    let desc = Box::leak(Box::new(BlockDesc {
        reserved: 0,
        size: std::mem::size_of::<VoidBlock>(),
    }));
    let block = Box::leak(Box::new(VoidBlock {
        isa: unsafe { &_NSConcreteStackBlock } as *const c_void,
        flags: 0,
        reserved: 0,
        invoke,
        descriptor: desc,
    }));
    unsafe { _Block_copy(block as *const VoidBlock as *const c_void) }
}

unsafe fn invoke_void(block: *mut c_void) {
    if block.is_null() {
        return;
    }
    unsafe {
        let head = &*(block as *const VoidBlock);
        let invoke: unsafe extern "C" fn(*mut c_void) = std::mem::transmute(head.invoke);
        invoke(block);
    }
}

unsafe fn invoke_usize(block: *mut c_void, value: usize) {
    if block.is_null() {
        return;
    }
    unsafe {
        let head = &*(block as *const VoidBlock);
        let invoke: unsafe extern "C" fn(*mut c_void, usize) = std::mem::transmute(head.invoke);
        invoke(block, value);
    }
}

fn msg0(obj: *mut c_void, sel: *mut c_void) -> *mut c_void {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel)
    }
}

fn msg_ptr(obj: *mut c_void, sel: *mut c_void, a: *mut c_void) {
    unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) =
            std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel, a);
    }
}

#[cfg(target_os = "ios")]
fn msg_u8(obj: *mut c_void, sel: *mut c_void, a: *mut c_void) -> u8 {
    if obj.is_null() {
        return 0;
    }
    unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> u8 =
            std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel, a)
    }
}

fn msg_ptr_ret(obj: *mut c_void, sel: *mut c_void, a: *mut c_void) -> *mut c_void {
    unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel, a)
    }
}

fn msg_ptr2(obj: *mut c_void, sel: *mut c_void, a: *mut c_void, b: *mut c_void) -> *mut c_void {
    unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel, a, b)
    }
}

fn msg3(
    obj: *mut c_void,
    sel: *mut c_void,
    a: *mut c_void,
    b: *mut c_void,
    c: *mut c_void,
) -> *mut c_void {
    unsafe {
        let f: unsafe extern "C" fn(
            *mut c_void,
            *mut c_void,
            *mut c_void,
            *mut c_void,
            *mut c_void,
        ) -> *mut c_void = std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel, a, b, c)
    }
}
