//! System open and save panels for OPML.
//!
//! macOS uses AppKit `NSOpenPanel` and `NSSavePanel` on the main thread.
//! Other systems, including iOS, return a structured "use the clipboard"
//! result. The iOS path is the same JSON shape, so a later
//! `UIDocumentPickerViewController` can replace the body without changing
//! the screen.

#[cfg(target_os = "macos")]
const MAX_OPML_BYTES: usize = 2_000_000;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn with_opml_extension(path: &str) -> String {
    let path = path.trim();
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".opml") || lower.ends_with(".xml") {
        path.to_string()
    } else if path.is_empty() {
        "Pulse.opml".into()
    } else {
        format!("{path}.opml")
    }
}

/// Reads an `.opml` or `.xml` file the user picks. `Ok(None)` is cancel.
/// The string pair is `(path, text)`.
pub fn pick_opml_text() -> Result<Option<(String, String)>, String> {
    #[cfg(target_os = "macos")]
    {
        return macos::pick_text();
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(unavailable())
    }
}

/// Writes `xml` where the user chooses. `Ok(None)` is cancel. The path is
/// what was written.
pub fn save_opml_text(xml: &str) -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    {
        return macos::save_text(xml);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = xml;
        Err(unavailable())
    }
}

#[cfg(not(target_os = "macos"))]
fn unavailable() -> String {
    "系统文件面板还没接到这个系统，请改用剪贴板。".into()
}

pub fn reply_cancelled() -> String {
    serde_json::json!({
        "ok": false,
        "cancelled": true,
        "path": "",
        "error": ""
    })
    .to_string()
}

pub fn reply_error(message: &str) -> String {
    serde_json::json!({
        "ok": false,
        "cancelled": false,
        "path": "",
        "error": message
    })
    .to_string()
}

pub fn reply_saved(path: &str) -> String {
    serde_json::json!({
        "ok": true,
        "cancelled": false,
        "path": path,
        "error": ""
    })
    .to_string()
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{with_opml_extension, MAX_OPML_BYTES};
    use std::ffi::{c_char, c_void, CStr, CString};
    use std::sync::Mutex;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send();
    }

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    #[link(name = "System")]
    unsafe extern "C" {
        /// `dispatch_get_main_queue()` is a header inline. libSystem exports this object instead.
        static _dispatch_main_q: c_void;
        fn dispatch_sync(queue: *mut c_void, block: *mut c_void);
        static _NSConcreteStackBlock: c_void;
        fn _Block_copy(block: *const c_void) -> *mut c_void;
    }

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

    const MODAL_OK: isize = 1;

    #[derive(Clone, Copy)]
    enum Kind {
        Open,
        Save,
    }

    enum Outcome {
        Cancel,
        Err(String),
        Text { path: String, text: String },
        Path(String),
    }

    struct Slot {
        kind: Kind,
        xml: String,
        result: Option<Outcome>,
    }

    fn slot() -> &'static Mutex<Slot> {
        use std::sync::OnceLock;
        static SLOT: OnceLock<Mutex<Slot>> = OnceLock::new();
        SLOT.get_or_init(|| {
            Mutex::new(Slot {
                kind: Kind::Open,
                xml: String::new(),
                result: None,
            })
        })
    }

    pub fn pick_text() -> Result<Option<(String, String)>, String> {
        match run(Kind::Open, String::new()) {
            Outcome::Cancel => Ok(None),
            Outcome::Err(error) => Err(error),
            Outcome::Text { path, text } => Ok(Some((path, text))),
            Outcome::Path(_) => Err("文件面板没有返回正文。".into()),
        }
    }

    pub fn save_text(xml: &str) -> Result<Option<String>, String> {
        match run(Kind::Save, xml.to_string()) {
            Outcome::Cancel => Ok(None),
            Outcome::Err(error) => Err(error),
            Outcome::Path(path) => Ok(Some(path)),
            Outcome::Text { .. } => Err("文件面板没有返回路径。".into()),
        }
    }

    fn run(kind: Kind, xml: String) -> Outcome {
        {
            let mut guard = slot().lock().unwrap_or_else(|poison| poison.into_inner());
            guard.kind = kind;
            guard.xml = xml;
            guard.result = None;
        }
        if is_main() {
            perform();
        } else {
            let block = heap_block(perform_block as *const c_void);
            let queue = std::ptr::addr_of!(_dispatch_main_q) as *mut c_void;
            unsafe { dispatch_sync(queue, block) };
        }
        slot()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .result
            .take()
            .unwrap_or_else(|| Outcome::Err("文件面板没有返回。".into()))
    }

    unsafe extern "C" fn perform_block(_block: *mut c_void) {
        perform();
    }

    fn perform() {
        let (kind, xml) = {
            let guard = slot().lock().unwrap_or_else(|poison| poison.into_inner());
            (guard.kind, guard.xml.clone())
        };
        let result = match kind {
            Kind::Open => open_panel(),
            Kind::Save => save_panel(&xml),
        };
        slot()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .result = Some(result);
    }

    fn open_panel() -> Outcome {
        activate();
        let panel = msg0(class("NSOpenPanel"), sel("openPanel"));
        if panel.is_null() {
            return Outcome::Err("打不开文件面板。".into());
        }
        msg_bool(panel, sel("setCanChooseFiles:"), true);
        msg_bool(panel, sel("setCanChooseDirectories:"), false);
        msg_bool(panel, sel("setAllowsMultipleSelection:"), false);
        msg_ptr(panel, sel("setAllowedFileTypes:"), file_types());
        msg_ptr(panel, sel("setTitle:"), ns_string("导入 OPML"));
        msg_ptr(panel, sel("setPrompt:"), ns_string("导入"));
        msg_ptr(
            panel,
            sel("setMessage:"),
            ns_string("选择一份 .opml 或 .xml 订阅列表。"),
        );
        if msg0_isize(panel, sel("runModal")) != MODAL_OK {
            return Outcome::Cancel;
        }
        let urls = msg0(panel, sel("URLs"));
        if msg0_usize(urls, sel("count")) == 0 {
            return Outcome::Cancel;
        }
        let url = msg_usize(urls, sel("objectAtIndex:"), 0);
        let path = ns_utf8(msg0(url, sel("path")));
        if path.is_empty() {
            return Outcome::Err("没有拿到文件路径。".into());
        }
        read_opml(&path)
    }

    fn save_panel(xml: &str) -> Outcome {
        activate();
        let panel = msg0(class("NSSavePanel"), sel("savePanel"));
        if panel.is_null() {
            return Outcome::Err("打不开保存面板。".into());
        }
        msg_ptr(panel, sel("setAllowedFileTypes:"), opml_type());
        msg_bool(panel, sel("setCanCreateDirectories:"), true);
        msg_bool(panel, sel("setExtensionHidden:"), false);
        msg_ptr(panel, sel("setNameFieldStringValue:"), ns_string("Pulse.opml"));
        msg_ptr(panel, sel("setTitle:"), ns_string("导出 OPML"));
        msg_ptr(panel, sel("setPrompt:"), ns_string("存储"));
        msg_ptr(panel, sel("setMessage:"), ns_string("把当前 RSS 订阅存成 .opml。"));
        if msg0_isize(panel, sel("runModal")) != MODAL_OK {
            return Outcome::Cancel;
        }
        let url = msg0(panel, sel("URL"));
        let path = with_opml_extension(&ns_utf8(msg0(url, sel("path"))));
        if path.is_empty() {
            return Outcome::Err("没有拿到保存路径。".into());
        }
        if let Err(error) = std::fs::write(&path, xml) {
            let _ = error;
            return Outcome::Err("写不了这个文件。".into());
        }
        Outcome::Path(path)
    }

    fn read_opml(path: &str) -> Outcome {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(_) => return Outcome::Err("读不了这个文件。".into()),
        };
        if bytes.len() > MAX_OPML_BYTES {
            return Outcome::Err("这份 OPML 太大了。".into());
        }
        match String::from_utf8(bytes) {
            Ok(text) => Outcome::Text {
                path: path.to_string(),
                text,
            },
            Err(_) => Outcome::Err("这份文件不是 UTF-8 文本。".into()),
        }
    }

    fn file_types() -> *mut c_void {
        let types = msg0(class("NSMutableArray"), sel("array"));
        msg_ptr(types, sel("addObject:"), ns_string("opml"));
        msg_ptr(types, sel("addObject:"), ns_string("xml"));
        types
    }

    fn opml_type() -> *mut c_void {
        let types = msg0(class("NSMutableArray"), sel("array"));
        msg_ptr(types, sel("addObject:"), ns_string("opml"));
        types
    }

    fn activate() {
        let app = msg0(class("NSApplication"), sel("sharedApplication"));
        msg_bool(app, sel("activateIgnoringOtherApps:"), true);
    }

    fn is_main() -> bool {
        msg0_u8(class("NSThread"), sel("isMainThread")) != 0
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
        msg_ptr_ret(
            class("NSString"),
            sel("stringWithUTF8String:"),
            c.as_ptr() as *mut c_void,
        )
    }

    fn ns_utf8(ns: *mut c_void) -> String {
        if ns.is_null() {
            return String::new();
        }
        let ptr = msg0(ns, sel("UTF8String")) as *const c_char;
        if ptr.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
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

    fn msg0_isize(obj: *mut c_void, sel: *mut c_void) -> isize {
        if obj.is_null() {
            return 0;
        }
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> isize =
                std::mem::transmute(objc_msg_send as *const ());
            f(obj, sel)
        }
    }

    fn msg0_usize(obj: *mut c_void, sel: *mut c_void) -> usize {
        if obj.is_null() {
            return 0;
        }
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize =
                std::mem::transmute(objc_msg_send as *const ());
            f(obj, sel)
        }
    }

    fn msg0_u8(obj: *mut c_void, sel: *mut c_void) -> u8 {
        if obj.is_null() {
            return 0;
        }
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> u8 =
                std::mem::transmute(objc_msg_send as *const ());
            f(obj, sel)
        }
    }

    fn msg_bool(obj: *mut c_void, sel: *mut c_void, value: bool) {
        if obj.is_null() {
            return;
        }
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, u8) =
                std::mem::transmute(objc_msg_send as *const ());
            f(obj, sel, u8::from(value));
        }
    }

    fn msg_ptr(obj: *mut c_void, sel: *mut c_void, arg: *mut c_void) {
        if obj.is_null() {
            return;
        }
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) =
                std::mem::transmute(objc_msg_send as *const ());
            f(obj, sel, arg);
        }
    }

    fn msg_ptr_ret(obj: *mut c_void, sel: *mut c_void, arg: *mut c_void) -> *mut c_void {
        if obj.is_null() {
            return std::ptr::null_mut();
        }
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void =
                std::mem::transmute(objc_msg_send as *const ());
            f(obj, sel, arg)
        }
    }

    fn msg_usize(obj: *mut c_void, sel: *mut c_void, index: usize) -> *mut c_void {
        if obj.is_null() {
            return std::ptr::null_mut();
        }
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> *mut c_void =
                std::mem::transmute(objc_msg_send as *const ());
            f(obj, sel, index)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_name_grows_an_opml_suffix() {
        assert_eq!(with_opml_extension("Pulse"), "Pulse.opml");
        assert_eq!(with_opml_extension("/tmp/feeds.OPML"), "/tmp/feeds.OPML");
        assert_eq!(with_opml_extension("/tmp/feeds.xml"), "/tmp/feeds.xml");
        assert_eq!(with_opml_extension(""), "Pulse.opml");
    }

    #[test]
    fn this_host_reports_whether_panels_exist() {
        if cfg!(target_os = "macos") {
            return;
        }
        let open = pick_opml_text();
        let save = save_opml_text("<opml/>");
        let message = open.expect_err("linux has no panel");
        assert!(message.contains("剪贴板"));
        assert!(save.expect_err("linux has no panel").contains("剪贴板"));
    }
}
