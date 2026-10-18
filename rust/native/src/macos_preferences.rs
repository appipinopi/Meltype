// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Existing NSUserDefaults schema, no migration/reset of stored values.
use std::ffi::c_void;
#[no_mangle]
pub extern "C" fn meltype_macos_mode_after(action: u32, current: u32) -> i32 {
    if action > 3 || current > 2 {
        return -1;
    }
    if action == 3 {
        ((current + 1) % 3) as i32
    } else {
        action as i32
    }
}
/// Read (operation 0) or write (1) the original indicator preference. -1=error.
/// # Safety
/// defaults is a live NSUserDefaults borrowed through this main-thread call.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_indicator_preference(
    defaults: *mut c_void,
    operation: u32,
    value: i32,
) -> i32 {
    if defaults.is_null() || operation > 1 {
        return -1;
    }
    #[cfg(target_os = "macos")]
    {
        native::indicator(defaults, operation, value != 0)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = value;
        -1
    }
}
/// Create +1 NSObject selection state, restoring existing stored mode and
/// registering automatic as the default. Null if existing value is invalid.
/// # Safety
/// defaults is a live NSUserDefaults borrowed through this main-thread call.
/// Caller consumes/releases returned +1 reference on main thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_mode_selection_create(defaults: *mut c_void) -> *mut c_void {
    if defaults.is_null() {
        return std::ptr::null_mut();
    }
    #[cfg(target_os = "macos")]
    {
        native::create(defaults)
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::ptr::null_mut()
    }
}
/// Read cached selection (0), or select/persist mode (1). -1=error.
/// # Safety
/// owner is a live native selection object. Main thread only.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_mode_selection(
    owner: *mut c_void,
    operation: u32,
    mode: u32,
) -> i32 {
    if owner.is_null() || operation > 1 || mode > 2 {
        return -1;
    }
    #[cfg(target_os = "macos")]
    {
        native::selection(owner, operation, mode)
    }
    #[cfg(not(target_os = "macos"))]
    {
        -1
    }
}
#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use objc2::{
        define_class, msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject, NSObject, NSObjectProtocol},
        AnyThread, DefinedClass, MainThreadMarker,
    };
    use objc2_foundation::NSString;
    use std::cell::Cell;
    struct State {
        defaults: Retained<AnyObject>,
        mode: Cell<u32>,
    }
    define_class!(
        #[unsafe(super=NSObject)]
        #[ivars=State]
        struct Selection;
        unsafe impl NSObjectProtocol for Selection {}
    );
    unsafe fn defaults(pointer: *mut c_void) -> Option<Retained<AnyObject>> {
        let object = Retained::retain(pointer.cast::<AnyObject>())?;
        let class = AnyClass::get(c"NSUserDefaults")?;
        let valid: bool = msg_send![&object,isKindOfClass:class];
        valid.then_some(object)
    }
    fn mode_name(mode: u32) -> &'static str {
        ["automatic", "direct", "code"][mode as usize]
    }
    pub unsafe fn indicator(pointer: *mut c_void, operation: u32, value: bool) -> i32 {
        if MainThreadMarker::new().is_none() {
            return -1;
        }
        let Some(defaults) = defaults(pointer) else {
            return -1;
        };
        let key = NSString::from_str("MacShowModeIndicator");
        if operation == 1 {
            let _: () = msg_send![&defaults,setBool:value,forKey:&*key];
            return i32::from(value);
        }
        let object: Option<Retained<AnyObject>> = msg_send![&defaults,objectForKey:&*key];
        if object.is_none() {
            return 1;
        }
        let value: bool = msg_send![&defaults,boolForKey:&*key];
        i32::from(value)
    }
    pub unsafe fn create(pointer: *mut c_void) -> *mut c_void {
        if MainThreadMarker::new().is_none() {
            return std::ptr::null_mut();
        }
        let Some(defaults) = defaults(pointer) else {
            return std::ptr::null_mut();
        };
        let key = NSString::from_str("MacSelectedInputMode");
        let automatic = NSString::from_str("automatic");
        let Some(dictionary) = AnyClass::get(c"NSDictionary") else {
            return std::ptr::null_mut();
        };
        let registration: Retained<AnyObject> =
            msg_send![dictionary,dictionaryWithObject:&*automatic,forKey:&*key];
        let _: () = msg_send![&defaults,registerDefaults:&*registration];
        let value: Option<Retained<NSString>> = msg_send![&defaults,stringForKey:&*key];
        let mode = match value.as_ref().map(|v| v.to_string()).as_deref() {
            Some("automatic") => 0,
            Some("direct") => 1,
            Some("code") => 2,
            _ => return std::ptr::null_mut(),
        };
        let this = Selection::alloc().set_ivars(State {
            defaults,
            mode: Cell::new(mode),
        });
        let this: Retained<Selection> = msg_send![super(this), init];
        Retained::into_raw(this).cast()
    }
    pub unsafe fn selection(pointer: *mut c_void, operation: u32, mode: u32) -> i32 {
        if MainThreadMarker::new().is_none() {
            return -1;
        }
        let Some(owner) = Retained::retain(pointer.cast::<AnyObject>())
            .and_then(|o| o.downcast::<Selection>().ok())
        else {
            return -1;
        };
        if operation == 1 {
            owner.ivars().mode.set(mode);
            let value = NSString::from_str(mode_name(mode));
            let key = NSString::from_str("MacSelectedInputMode");
            let _: () = msg_send![&owner.ivars().defaults,setObject:&*value,forKey:&*key];
        }
        owner.ivars().mode.get() as i32
    }
}
