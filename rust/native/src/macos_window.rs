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
#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use objc2::{
        msg_send,
        rc::Retained,
        runtime::{AnyObject, Sel},
        sel, MainThreadMarker,
    };
    use objc2_foundation::{NSAttributedString, NSString};
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
                if !responds(&window, sel!(update)) || !responds(&window, sel!(show:)) {
                    return 0;
                }
                let _: () = msg_send![&window, update];
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
