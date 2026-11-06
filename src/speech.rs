//! Chinese speech for the morning script.
//!
//! NSH 0.3.0 has no speech component. On Mac and iOS this uses
//! `AVSpeechSynthesizer` (AVFoundation), which is on the deployment floor.
//! Other hosts keep the script on screen and say so.

pub fn speak(text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("还没有稿子。".into());
    }
    let text: String = text.chars().take(1800).collect();
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        apple::speak(&text)
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let _ = text;
        Err("这台系统没有中文语音。稿子还在。".into())
    }
}

pub fn stop() {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    apple::stop();
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple {
    use std::ffi::{c_char, c_void, CString};
    use std::sync::atomic::{AtomicPtr, Ordering};

    #[link(name = "objc")]
    #[allow(non_snake_case)]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send();
    }

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {}

    static SYNTH: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

    pub fn speak(text: &str) -> Result<(), String> {
        let synth = synthesizer().ok_or("这台系统没有中文语音。稿子还在。")?;
        if speaking(synth) {
            let _ = msg_usize(synth, sel("stopSpeakingAtBoundary:"), 0);
        }
        let utterance = msg_ptr(
            class("AVSpeechUtterance"),
            sel("speechUtteranceWithString:"),
            ns_string(text),
        );
        if utterance.is_null() {
            return Err("没有读出来。".into());
        }
        let voice = msg_ptr(
            class("AVSpeechSynthesisVoice"),
            sel("voiceWithLanguage:"),
            ns_string("zh-CN"),
        );
        if !voice.is_null() {
            msg_ptr(utterance, sel("setVoice:"), voice);
        }
        msg_f32(utterance, sel("setRate:"), 0.48);
        msg_ptr(synth, sel("speakUtterance:"), utterance);
        eprintln!("[pulse] briefing speak chars={}", text.chars().count());
        Ok(())
    }

    pub fn stop() {
        let Some(synth) = synthesizer() else {
            return;
        };
        if speaking(synth) {
            let _ = msg_usize(synth, sel("stopSpeakingAtBoundary:"), 0);
        }
    }

    fn synthesizer() -> Option<*mut c_void> {
        let existing = SYNTH.load(Ordering::SeqCst);
        if !existing.is_null() {
            return Some(existing);
        }
        if class("AVSpeechSynthesizer").is_null() {
            return None;
        }
        let created = msg0(msg0(class("AVSpeechSynthesizer"), sel("alloc")), sel("init"));
        if created.is_null() {
            return None;
        }
        match SYNTH.compare_exchange(std::ptr::null_mut(), created, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => Some(created),
            Err(current) => Some(current),
        }
    }

    fn speaking(synth: *mut c_void) -> bool {
        msg_u8(synth, sel("isSpeaking")) != 0
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

    fn msg_f32(obj: *mut c_void, sel: *mut c_void, arg: f32) {
        if obj.is_null() {
            return;
        }
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, f32) =
                std::mem::transmute(objc_msg_send as *const ());
            f(obj, sel, arg);
        }
    }

    fn msg_usize(obj: *mut c_void, sel: *mut c_void, arg: usize) -> u8 {
        if obj.is_null() {
            return 0;
        }
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> u8 =
                std::mem::transmute(objc_msg_send as *const ());
            f(obj, sel, arg)
        }
    }

    fn msg_u8(obj: *mut c_void, sel: *mut c_void) -> u8 {
        if obj.is_null() {
            return 0;
        }
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> u8 =
                std::mem::transmute(objc_msg_send as *const ());
            f(obj, sel)
        }
    }
}
