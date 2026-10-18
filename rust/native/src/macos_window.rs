// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Direct Objective-C candidate window operations. No locks span OS calls.
use std::ffi::c_void;
/// Ops: 0 isVisible, 1 update/show (NSUInteger location hint), 2 responds to
/// direction (+/-1), 3 perform direction, 4 hide, 5 annotation if visible.
/// Return 1=success/true, 0=unsupported/hidden/rejected. Non-macOS always rejects.
/// # Safety
/// window is a live NSObject implementing the IMKCandidates method signatures,
/// borrowed through this call. UTF-16 text is readable for length units (null
/// only if length=0); callbacks must not throw through Rust. Call on main thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_candidate_window(
    window: *mut c_void,
    operation: u32,
    argument: i64,
    text: *const u16,
    length: u32,
) -> i32 {
    if window.is_null() || operation > 5 {
        return 0;
    }
    #[cfg(target_os = "macos")]
    {
        macos::operate(window, operation, argument, text, length)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (argument, text, length);
        0
    }
}
/// Create an IMKServer through the SDK initializer on the main thread.
/// Return a +1 Objective-C reference (caller consumes/releases it), or null.
/// This opens the application's private server; it does not install/enable an IME.
/// # Safety
/// Both UTF-16 buffers are readable for their stated lengths; no exception may
/// cross this call. The returned reference must be released once on main thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_server_create(
    name: *const u16,
    name_length: u32,
    bundle: *const u16,
    bundle_length: u32,
) -> *mut c_void {
    #[cfg(target_os = "macos")]
    {
        macos::server_create(name, name_length, bundle, bundle_length)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (name, name_length, bundle, bundle_length);
        std::ptr::null_mut()
    }
}
/// Create an IMKCandidates panel with the SDK default initializer/settings.
/// Return a +1 Objective-C reference for the caller, or null. Main thread only.
/// # Safety
/// server is a live borrowed NSObject; if it is an IMKServer, it must remain
/// valid through the initializer. Caller releases returned reference exactly once.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_candidates_create(
    server: *mut c_void,
    panel_type: u64,
) -> *mut c_void {
    if server.is_null() {
        return std::ptr::null_mut();
    }
    #[cfg(target_os = "macos")]
    {
        macos::candidates_create(server, panel_type)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = panel_type;
        std::ptr::null_mut()
    }
}
#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use objc2::{
        msg_send,
        rc::{Allocated, Retained},
        runtime::{AnyClass, AnyObject, Sel},
        sel, MainThreadMarker,
    };
    use objc2_foundation::{NSAttributedString, NSString};
    // Register the SDK's Objective-C classes even when loaded by a plain C host.
    #[link(name = "InputMethodKit", kind = "framework")]
    extern "C" {}
    unsafe fn string(text: *const u16, length: u32) -> Option<Retained<NSString>> {
        if text.is_null() && length != 0 {
            return None;
        }
        let units = if length == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(text, length as usize)
        };
        let text = String::from_utf16(units).ok()?;
        Some(NSString::from_str(&text))
    }
    pub unsafe fn server_create(
        name: *const u16,
        name_length: u32,
        bundle: *const u16,
        bundle_length: u32,
    ) -> *mut c_void {
        if MainThreadMarker::new().is_none() {
            return std::ptr::null_mut();
        }
        let Some(name) = string(name, name_length) else {
            return std::ptr::null_mut();
        };
        let Some(bundle) = string(bundle, bundle_length) else {
            return std::ptr::null_mut();
        };
        if name_length == 0 || bundle_length == 0 {
            return std::ptr::null_mut();
        }
        let Some(class) = AnyClass::get(c"IMKServer") else {
            return std::ptr::null_mut();
        };
        let allocated: Allocated<AnyObject> = msg_send![class, alloc];
        let server: Option<Retained<AnyObject>> = msg_send![allocated,
            initWithName: &*name, bundleIdentifier: &*bundle];
        server.map_or(std::ptr::null_mut(), |value| {
            Retained::into_raw(value).cast()
        })
    }
    pub unsafe fn candidates_create(server: *mut c_void, panel_type: u64) -> *mut c_void {
        if MainThreadMarker::new().is_none() {
            return std::ptr::null_mut();
        }
        let Some(server_class) = AnyClass::get(c"IMKServer") else {
            return std::ptr::null_mut();
        };
        let Some(class) = AnyClass::get(c"IMKCandidates") else {
            return std::ptr::null_mut();
        };
        let Some(server) = Retained::retain(server.cast::<AnyObject>()) else {
            return std::ptr::null_mut();
        };
        let valid: bool = msg_send![&server, isKindOfClass: server_class];
        if !valid {
            return std::ptr::null_mut();
        }
        let Ok(panel_type) = usize::try_from(panel_type) else {
            return std::ptr::null_mut();
        };
        let allocated: Allocated<AnyObject> = msg_send![class, alloc];
        let panel: Option<Retained<AnyObject>> = msg_send![allocated,
            initWithServer: &*server, panelType: panel_type];
        panel.map_or(std::ptr::null_mut(), |value| {
            Retained::into_raw(value).cast()
        })
    }
    unsafe fn responds(window: &AnyObject, selector: Sel) -> bool {
        msg_send![window, respondsToSelector: selector]
    }
    pub unsafe fn operate(
        window: *mut c_void,
        operation: u32,
        argument: i64,
        text: *const u16,
        length: u32,
    ) -> i32 {
        if MainThreadMarker::new().is_none() {
            return 0;
        }
        // Pin across synchronous reentrant SDK calls. Caller retains its borrow.
        let Some(window) = Retained::retain(window.cast::<AnyObject>()) else {
            return 0;
        };
        match operation {
            0 => {
                if !responds(&window, sel!(isVisible)) {
                    return 0;
                }
                let visible: bool = msg_send![&window, isVisible];
                i32::from(visible)
            }
            1 => {
                let Ok(hint) = usize::try_from(argument) else {
                    return 0;
                };
                if !responds(&window, sel!(updateCandidates)) || !responds(&window, sel!(show:)) {
                    return 0;
                }
                let _: () = msg_send![&window, updateCandidates];
                let _: () = msg_send![&window, show: hint];
                1
            }
            2 | 3 => {
                if argument != -1 && argument != 1 {
                    return 0;
                }
                let step = if argument > 0 {
                    sel!(moveDown:)
                } else {
                    sel!(moveUp:)
                };
                if !responds(&window, step) {
                    return 0;
                }
                if operation == 3 {
                    if !responds(&window, sel!(performSelector:withObject:)) {
                        return 0;
                    }
                    let _: *mut AnyObject = msg_send![&window, performSelector: step,
                        withObject: std::ptr::null::<AnyObject>()];
                }
                1
            }
            4 => {
                if !responds(&window, sel!(hide)) {
                    return 0;
                }
                let _: () = msg_send![&window, hide];
                1
            }
            5 => {
                // Validate the entire payload before asking the SDK to change UI.
                if text.is_null() && length != 0 {
                    return 0;
                }
                let units = if length == 0 {
                    &[][..]
                } else {
                    std::slice::from_raw_parts(text, length as usize)
                };
                let Ok(text) = String::from_utf16(units) else {
                    return 0;
                };
                if !responds(&window, sel!(isVisible)) || !responds(&window, sel!(showAnnotation:))
                {
                    return 0;
                }
                let visible: bool = msg_send![&window, isVisible];
                if !visible {
                    return 0;
                }
                let value = NSAttributedString::from_nsstring(&NSString::from_str(&text));
                let _: () = msg_send![&window, showAnnotation: &*value];
                1
            }
            _ => 0,
        }
    }
}
