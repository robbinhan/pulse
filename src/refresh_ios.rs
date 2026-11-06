//! iOS Background App Refresh (`BGAppRefreshTask`, `UIBackgroundModes` = fetch).
//!
//! Registration has to happen during launch. The handler polls a few due RSS
//! feeds and posts the same local notices and badge as a foreground refresh.
//! If `BGTaskScheduler` is missing, or the packaged plist has no permitted
//! identifier, this logs once and returns. Sideload builds often hit that
//! path until `scripts/ios-background-plist.py` has patched `Info.plist`.
//!
//! TODO: a notification after the user force-quits the app needs APNs
//! (Apple Developer account, push certificate, and a server). Do not invent
//! that backend here. Background refresh is best-effort and the system may
//! never grant a wake.

use std::ffi::{c_char, c_void, CString};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Once;

use crate::logic::refresh::{EARLIEST_SECS, TASK_ID};

#[link(name = "objc")]
#[allow(non_snake_case)]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    static _NSConcreteStackBlock: c_void;
    fn _Block_copy(block: *const c_void) -> *mut c_void;
    #[link_name = "objc_msgSend"]
    fn objc_msg_send();
}

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}
#[link(name = "BackgroundTasks", kind = "framework")]
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

static REGISTERED: AtomicBool = AtomicBool::new(false);
static DONE: AtomicBool = AtomicBool::new(false);
static TASK: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

pub fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if register() {
            REGISTERED.store(true, Ordering::SeqCst);
            let scheduled = schedule();
            eprintln!(
                "[pulse] background refresh mode=fetch task={TASK_ID} scheduled={}",
                u8::from(scheduled)
            );
        } else {
            eprintln!(
                "[pulse] background refresh unavailable task={TASK_ID} (no BGTaskScheduler, or Info.plist has no UIBackgroundModes fetch / BGTaskSchedulerPermittedIdentifiers). Local notices still fire while the app is open."
            );
        }
    });
}

/// Submits the next wake. No-op until [`install`] registered the identifier.
pub fn schedule() -> bool {
    if !REGISTERED.load(Ordering::SeqCst) {
        return false;
    }
    let request_class = class("BGAppRefreshTaskRequest");
    let scheduler = msg0(class("BGTaskScheduler"), sel("sharedScheduler"));
    if request_class.is_null() || scheduler.is_null() {
        return false;
    }
    let request = msg_ptr(msg0(request_class, sel("alloc")), sel("initWithIdentifier:"), ns_string(TASK_ID));
    if request.is_null() {
        return false;
    }
    let when = msg_f64(class("NSDate"), sel("dateWithTimeIntervalSinceNow:"), EARLIEST_SECS);
    msg_ptr(request, sel("setEarliestBeginDate:"), when);
    let mut error: *mut c_void = std::ptr::null_mut();
    let ok = msg_out_bool(scheduler, sel("submitTaskRequest:error:"), request, &mut error);
    if !ok {
        eprintln!("[pulse] background refresh schedule failed task={TASK_ID}");
    }
    ok
}

fn register() -> bool {
    let scheduler_class = class("BGTaskScheduler");
    if scheduler_class.is_null() {
        return false;
    }
    let scheduler = msg0(scheduler_class, sel("sharedScheduler"));
    if scheduler.is_null() {
        return false;
    }
    let block = heap_block(launch_invoke as *const c_void);
    msg_bool3(
        scheduler,
        sel("registerForTaskWithIdentifier:usingQueue:launchHandler:"),
        ns_string(TASK_ID),
        std::ptr::null_mut(),
        block,
    )
}

unsafe extern "C" fn launch_invoke(_block: *mut c_void, task: *mut c_void) {
    DONE.store(false, Ordering::SeqCst);
    TASK.store(task, Ordering::SeqCst);
    let expire = heap_block(expire_invoke as *const c_void);
    msg_ptr(task, sel("setExpirationHandler:"), expire);
    crate::store::begin_background_poll();
    let ok = crate::store::background_poll();
    finish(ok);
}

unsafe extern "C" fn expire_invoke(_block: *mut c_void) {
    crate::store::mark_background_expired();
    finish(false);
}

fn finish(ok: bool) {
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    let task = TASK.swap(std::ptr::null_mut(), Ordering::SeqCst);
    if !task.is_null() {
        msg_u8(task, sel("setTaskCompletedWithSuccess:"), u8::from(ok));
    }
    let _ = schedule();
}

fn class(name: &str) -> *mut c_void {
    let Ok(c) = CString::new(name) else {
        return std::ptr::null_mut();
    };
    unsafe { objc_getClass(c.as_ptr()) }
}

fn sel(name: &str) -> *mut c_void {
    let Ok(c) = CString::new(name) else {
        return std::ptr::null_mut();
    };
    unsafe { sel_registerName(c.as_ptr()) }
}

fn ns_string(text: &str) -> *mut c_void {
    let Ok(c) = CString::new(text) else {
        return msg0(class("NSString"), sel("string"));
    };
    msg_ptr(class("NSString"), sel("stringWithUTF8String:"), c.as_ptr() as *mut c_void)
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

fn msg_ptr(obj: *mut c_void, sel: *mut c_void, arg: *mut c_void) -> *mut c_void {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel, arg)
    }
}

fn msg_f64(obj: *mut c_void, sel: *mut c_void, arg: f64) -> *mut c_void {
    if obj.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void, f64) -> *mut c_void =
            std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel, arg)
    }
}

fn msg_u8(obj: *mut c_void, sel: *mut c_void, arg: u8) {
    if obj.is_null() {
        return;
    }
    unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void, u8) =
            std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel, arg);
    }
}

fn msg_bool3(obj: *mut c_void, sel: *mut c_void, a: *mut c_void, b: *mut c_void, c: *mut c_void) -> bool {
    if obj.is_null() {
        return false;
    }
    unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void, *mut c_void) -> u8 =
            std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel, a, b, c) != 0
    }
}

fn msg_out_bool(obj: *mut c_void, sel: *mut c_void, arg: *mut c_void, error: *mut *mut c_void) -> bool {
    if obj.is_null() {
        return false;
    }
    unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut *mut c_void) -> u8 =
            std::mem::transmute(objc_msg_send as *const ());
        f(obj, sel, arg, error) != 0
    }
}
