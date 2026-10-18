// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use crate::{run, System};
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
    AnyThread,
};
use objc2_foundation::NSString;
use std::{
    ffi::{c_void, CStr},
    io::Write,
};
use unicode_normalization::UnicodeNormalization;
#[link(name = "Foundation", kind = "framework")]
extern "C" {}
#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISRegisterInputSource(bundle: *const c_void) -> i32;
    fn TISCreateInputSourceList(filter: *const c_void, include_all: u8) -> *const c_void;
    fn TISGetInputSourceProperty(source: *const c_void, key: *const c_void) -> *const c_void;
    static kTISPropertyBundleID: *const c_void;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: *const c_void);
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
}
unsafe fn class(name: &CStr) -> &'static AnyClass {
    AnyClass::get(name).expect("Foundation class unavailable")
}
// Copy UTF-16 directly: initWithBytes(.utf8) can consume a leading BOM.
fn string(value: &str) -> Retained<NSString> {
    let units: Vec<u16> = value.encode_utf16().collect();
    unsafe { msg_send![NSString::alloc(),initWithCharacters:units.as_ptr(),length:units.len()] }
}
unsafe fn url(path: &str) -> Retained<AnyObject> {
    let path = string(path);
    msg_send![class(c"NSURL"),fileURLWithPath:&*path]
}
struct List(*const c_void);
impl Drop for List {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}
fn restart(program: &str, argument: &str) -> Result<(), String> {
    unsafe {
        let task: Retained<AnyObject> = msg_send![class(c"NSTask"), new];
        let executable = url(program);
        let _: () = msg_send![&task,setExecutableURL:&*executable];
        let argument = string(argument);
        let arguments: Retained<AnyObject> =
            msg_send![class(c"NSArray"),arrayWithObject:&*argument];
        let _: () = msg_send![&task,setArguments:&*arguments];
        let null: Retained<AnyObject> = msg_send![class(c"NSFileHandle"), fileHandleWithNullDevice];
        let _: () = msg_send![&task,setStandardOutput:&*null];
        let _: () = msg_send![&task,setStandardError:&*null];
        let mut failure: *mut AnyObject = std::ptr::null_mut();
        let started: bool = msg_send![&task,launchAndReturnError:&mut failure];
        if !started {
            let description: Retained<NSString> = if let Some(error) = failure.as_ref() {
                msg_send![error, description]
            } else {
                string("Process launch failed")
            };
            return Err(description.to_string());
        }
        let _: () = msg_send![&task, waitUntilExit];
        // The original helper ignores killall's exit status.
        Ok(())
    }
}
struct Native;
impl System for Native {
    fn default_app_path(&mut self) -> String {
        unsafe {
            let bundle: Retained<AnyObject> = msg_send![class(c"NSBundle"), mainBundle];
            let path: Retained<NSString> = msg_send![&bundle, bundlePath];
            path.to_string()
        }
    }
    fn exists(&mut self, path: &str) -> bool {
        unsafe {
            let manager: Retained<AnyObject> = msg_send![class(c"NSFileManager"), defaultManager];
            let path = string(path);
            msg_send![&manager,fileExistsAtPath:&*path]
        }
    }
    fn register(&mut self, path: &str) -> i32 {
        unsafe {
            let url = url(path);
            TISRegisterInputSource(Retained::as_ptr(&url).cast())
        }
    }
    fn bundle_id(&mut self, path: &str) -> Option<String> {
        unsafe {
            let path = string(path);
            let bundle: Option<Retained<AnyObject>> =
                msg_send![class(c"NSBundle"),bundleWithPath:&*path];
            let bundle = bundle?;
            let id: Option<Retained<NSString>> = msg_send![&bundle, bundleIdentifier];
            id.map(|id| id.to_string())
        }
    }
    fn enabled(&mut self, id: &str) -> bool {
        unsafe {
            let list = TISCreateInputSourceList(std::ptr::null(), 0);
            if list.is_null() {
                return false;
            }
            let list = List(list);
            for index in 0..CFArrayGetCount(list.0) {
                let source = CFArrayGetValueAtIndex(list.0, index);
                let value = TISGetInputSourceProperty(source, kTISPropertyBundleID);
                if let Some(value) = value.cast::<NSString>().as_ref() {
                    if value.to_string().nfd().eq(id.nfd()) {
                        return true;
                    }
                }
            }
            false
        }
    }
    fn wait(&mut self) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    fn restart_agent(&mut self) -> Result<(), String> {
        restart("/usr/bin/killall", "TextInputMenuAgent")
    }
    fn stdout(&mut self, text: &str) {
        let _ = std::io::stdout().write_all(text.as_bytes());
    }
    fn stderr(&mut self, text: &str) {
        let _ = std::io::stderr().write_all(text.as_bytes());
    }
}
pub fn native_main() -> i32 {
    let path = std::env::args_os()
        .nth(1)
        .map(|arg| arg.to_string_lossy().into_owned());
    objc2::rc::autoreleasepool(|_| run(&mut Native, path))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_foundation_process_success_nonzero_and_launch_error() {
        objc2::rc::autoreleasepool(|_| {
            // Owned child processes only: never run killall or Carbon here.
            assert!(restart("/usr/bin/true", "fixture").is_ok());
            assert!(restart("/usr/bin/false", "fixture").is_ok());
            let missing =
                std::env::temp_dir().join(format!("Meltype-no-process-{}", std::process::id()));
            assert!(restart(&missing.to_string_lossy(), "fixture").is_err());
        });
    }
    #[test]
    fn foundation_utf16_path_preserves_bom_nul_and_supplementary_text() {
        objc2::rc::autoreleasepool(|_| {
            for value in ["", "\u{feff}path", "a\0b", "辞書 👩‍💻"] {
                assert_eq!(string(value).to_string(), value);
            }
        });
    }
}
