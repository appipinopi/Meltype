// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Original AppKit suggestion panel, constructed and placed by Rust.
use std::ffi::c_void;
/// Update/reuse the borrowed NSPanel, returning a +1 reference for caller ARC.
/// Missing suggestion/selection or invalid caret hides the existing panel.
/// # Safety
/// panel is null or a live NSPanel. text is readable UTF-16 for length units;
/// caret is readable for four doubles (x,y,width,height). Main thread only.
/// Caller consumes the result's +1 reference once. No OS exception may cross ABI.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_suggestion(
    panel: *mut c_void,
    text: *const u16,
    length: u32,
    present: i32,
    caret: *const f64,
    selected: i32,
) -> *mut c_void {
    #[cfg(target_os = "macos")]
    {
        native::update(panel, text, length, present != 0, caret, selected != 0)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (panel, text, length, present, caret, selected);
        std::ptr::null_mut()
    }
}
#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use objc2::{
        msg_send,
        rc::{Allocated, Retained},
        runtime::{AnyClass, AnyObject},
        MainThreadMarker,
    };
    use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString};
    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGRectIntersectsRect(a: NSRect, b: NSRect) -> bool;
        fn CGRectGetMinX(rect: NSRect) -> f64;
        fn CGRectGetMinY(rect: NSRect) -> f64;
        fn CGRectGetWidth(rect: NSRect) -> f64;
        fn CGRectGetHeight(rect: NSRect) -> f64;
    }
    unsafe fn class(name: &std::ffi::CStr) -> &'static AnyClass {
        AnyClass::get(name).expect("AppKit SDK class unavailable")
    }
    pub unsafe fn update(
        panel: *mut c_void,
        text: *const u16,
        length: u32,
        present: bool,
        caret: *const f64,
        selected: bool,
    ) -> *mut c_void {
        if MainThreadMarker::new().is_none() {
            return std::ptr::null_mut();
        }
        let mut panel = Retained::retain(panel.cast::<AnyObject>());
        if let Some(panel) = &panel {
            let valid: bool = msg_send![panel, isKindOfClass: class(c"NSPanel")];
            if !valid {
                return std::ptr::null_mut();
            }
        }
        let bounds = if caret.is_null() {
            None
        } else {
            let v = std::slice::from_raw_parts(caret, 4);
            let rect = NSRect::new(NSPoint::new(v[0], v[1]), NSSize::new(v[2], v[3]));
            if CGRectGetMinX(rect).is_finite()
                && CGRectGetMinY(rect).is_finite()
                && CGRectGetWidth(rect).is_finite()
                && CGRectGetHeight(rect).is_finite()
                && CGRectGetHeight(rect) > 0.0
            {
                Some(rect)
            } else {
                None
            }
        };
        if !present || !selected || bounds.is_none() {
            if let Some(panel) = &panel {
                let _: () = msg_send![panel, orderOut: std::ptr::null::<AnyObject>()];
            }
            return panel.map_or(std::ptr::null_mut(), |p| Retained::into_raw(p).cast());
        }
        if text.is_null() && length != 0 {
            return std::ptr::null_mut();
        }
        let units = if length == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(text, length as usize)
        };
        let Ok(text) = String::from_utf16(units) else {
            return std::ptr::null_mut();
        };
        let string = NSString::from_str(&text);
        let label: Retained<AnyObject> =
            msg_send![class(c"NSTextField"), labelWithString: &*string];
        let font: Retained<AnyObject> = msg_send![class(c"NSFont"), systemFontOfSize: 13.0f64];
        let _: () = msg_send![&label, setFont: &*font];
        let _: () = msg_send![&label, sizeToFit];
        let frame: NSRect = msg_send![&label, frame];
        let size = NSSize::new(frame.size.width + 24.0, frame.size.height + 16.0);
        if panel.is_none() {
            let allocated: Allocated<AnyObject> = msg_send![class(c"NSPanel"), alloc];
            panel = msg_send![allocated, initWithContentRect: NSRect::new(NSPoint::new(0.0,0.0),size),
                styleMask: 128usize, backing: 2usize, defer: false];
        }
        let Some(panel) = panel else {
            return std::ptr::null_mut();
        };
        let _: () = msg_send![&panel, setLevel: 3isize];
        let _: () = msg_send![&panel, setHasShadow: true];
        let color: Retained<AnyObject> = msg_send![class(c"NSColor"), windowBackgroundColor];
        let _: () = msg_send![&panel, setBackgroundColor: &*color];
        let _: () = msg_send![&panel, setIgnoresMouseEvents: true];
        let _: () = msg_send![&panel, setContentSize: size];
        let content: Option<Retained<AnyObject>> = msg_send![&panel, contentView];
        if let Some(content) = content {
            let children: Retained<NSArray<AnyObject>> = msg_send![&content, subviews];
            for child in children.iter() {
                let _: () = msg_send![&child, removeFromSuperview];
            }
            let _: () = msg_send![&label, setFrameOrigin: NSPoint::new(12.0,8.0)];
            let _: () = msg_send![&content, addSubview: &*label];
        }
        let caret = bounds.unwrap();
        let screens: Retained<NSArray<AnyObject>> = msg_send![class(c"NSScreen"), screens];
        let mut screen = None;
        for candidate in screens.iter() {
            let frame: NSRect = msg_send![&candidate, frame];
            if CGRectIntersectsRect(frame, caret) {
                screen = Some(candidate);
                break;
            }
        }
        let screen = screen.or_else(|| msg_send![class(c"NSScreen"), mainScreen]);
        if let Some(screen) = screen {
            let visible: NSRect = msg_send![&screen, visibleFrame];
            let min_x = CGRectGetMinX(caret);
            let min_y = CGRectGetMinY(caret);
            let x = visible
                .origin
                .x
                .max(min_x.min(visible.origin.x + visible.size.width - size.width));
            let y = visible.origin.y.max(
                (min_y - size.height - 6.0)
                    .min(visible.origin.y + visible.size.height - size.height),
            );
            let _: () = msg_send![&panel, setFrameOrigin: NSPoint::new(x,y)];
        }
        let _: () = msg_send![&panel, orderFrontRegardless];
        Retained::into_raw(panel).cast()
    }
}
