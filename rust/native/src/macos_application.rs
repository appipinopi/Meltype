// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! AppKit event loop and the original named Services registration.
use std::ffi::c_void;
/// Run the shared NSApplication on the main thread. Blocks until AppKit stops.
/// Return 1 after normal return, 0 on wrong thread/unavailable SDK/non-macOS.
#[no_mangle]
pub extern "C" fn meltype_macos_application_run() -> i32 {
    #[cfg(target_os = "macos")]
    unsafe {
        macos::run()
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}
/// Call the original NSRegisterServicesProvider on the main thread.
/// Return 1 when called (the SDK function returns void), 0 when rejected.
/// # Safety
/// provider is a live NSObject borrowed during this call. Caller keeps it alive
/// while registered, as with the SDK function. port is readable UTF-16 for length
/// units, or null for an empty string; no Objective-C exception may cross Rust.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_services_register(
    provider: *mut c_void,
    port: *const u16,
    length: u32,
) -> i32 {
    if provider.is_null() || length == 0 || port.is_null() {
        return 0;
    }
    #[cfg(target_os = "macos")]
    {
        macos::register(provider, port, length)
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}
#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use objc2::{
        msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject},
        MainThreadMarker,
    };
    use objc2_foundation::NSString;
    #[link(name = "AppKit", kind = "framework")]
    extern "C" {
        fn NSRegisterServicesProvider(provider: *mut AnyObject, name: *const NSString);
    }
    pub unsafe fn run() -> i32 {
        if MainThreadMarker::new().is_none() {
            return 0;
        }
        let Some(class) = AnyClass::get(c"NSApplication") else {
            return 0;
        };
        let app: Option<Retained<AnyObject>> = msg_send![class, sharedApplication];
        let Some(app) = app else {
            return 0;
        };
        // Hold across the complete SDK event loop and its reentrant callbacks.
        let _: () = msg_send![&app, run];
        1
    }
    pub unsafe fn register(provider: *mut c_void, port: *const u16, length: u32) -> i32 {
        if MainThreadMarker::new().is_none() {
            return 0;
        }
        let Ok(port) = String::from_utf16(std::slice::from_raw_parts(port, length as usize)) else {
            return 0;
        };
        // Services names cannot contain an embedded terminator.
        if port.contains('\0') {
            return 0;
        }
        let Some(provider) = Retained::retain(provider.cast::<AnyObject>()) else {
            return 0;
        };
        let name = NSString::from_str(&port);
        NSRegisterServicesProvider(Retained::as_ptr(&provider).cast_mut(), &*name);
        1
    }
}
