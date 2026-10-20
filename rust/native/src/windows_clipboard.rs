// SPDX-License-Identifier: GPL-3.0-or-later
//! One bounded read on explicit conversion, never a clipboard watcher.
use std::ffi::c_void;
#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(owner: *mut c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn RegisterClipboardFormatW(name: *const u16) -> u32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn GetClipboardData(format: u32) -> *mut c_void;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalLock(memory: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(memory: *mut c_void) -> i32;
    fn GlobalSize(memory: *mut c_void) -> usize;
}
struct Clipboard;
impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}
struct Memory(*mut c_void);
impl Drop for Memory {
    fn drop(&mut self) {
        unsafe {
            GlobalUnlock(self.0);
        }
    }
}
pub fn read() -> Option<String> {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let _clipboard = Clipboard;
        // The explicit exclusion always wins. Zero or malformed history/share
        // permission markers also fail closed before text is requested.
        for (name, always) in [
            ("ExcludeClipboardContentFromMonitorProcessing", true),
            ("CanIncludeInClipboardHistory", false),
            ("CanUploadToCloudClipboard", false),
        ] {
            let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
            let format = RegisterClipboardFormatW(wide.as_ptr());
            if format == 0 {
                return None;
            }
            if IsClipboardFormatAvailable(format) != 0 {
                if always {
                    return None;
                }
                let handle = GetClipboardData(format);
                if handle.is_null() || GlobalSize(handle) < 4 {
                    return None;
                }
                let bytes = GlobalLock(handle).cast::<u32>();
                if bytes.is_null() {
                    return None;
                }
                let _locked = Memory(handle);
                if bytes.read_unaligned() == 0 {
                    return None;
                }
            }
        }
        let handle = GetClipboardData(13); // CF_UNICODETEXT
        if handle.is_null() {
            return None;
        }
        let size = GlobalSize(handle) / 2;
        if size == 0 || size > 4096 {
            return None;
        }
        let pointer = GlobalLock(handle).cast::<u16>();
        if pointer.is_null() {
            return None;
        }
        let _locked = Memory(handle);
        let units = std::slice::from_raw_parts(pointer, size);
        let length = units.iter().position(|u| *u == 0)?;
        meltype_core::number_candidates::clipboard_line(&String::from_utf16(&units[..length]).ok()?)
    }
}
