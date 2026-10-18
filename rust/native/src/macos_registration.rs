// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Existing HIToolbox repair/backup and Carbon registration/enable ordering.
use std::ffi::c_void;
#[cfg(any(target_os = "macos", test))]
const IDENTIFIER: &str = "io.github.yksr-melt.inputmethod.Meltype";
#[cfg(any(target_os = "macos", test))]
fn owned_identifier(id: &str) -> bool {
    id == IDENTIFIER || id == format!("{IDENTIFIER}.Japanese")
}
#[cfg(any(target_os = "macos", test))]
trait InputSources {
    type List;
    fn register(&mut self) -> i32;
    fn list(&mut self) -> Option<Self::List>;
    fn count(&self, list: &Self::List) -> usize;
    fn identifier(&self, list: &Self::List, index: usize) -> Option<String>;
    fn enable(&mut self, list: &Self::List, index: usize) -> i32;
}
#[cfg(any(target_os = "macos", test))]
fn register_sources(api: &mut impl InputSources) -> i32 {
    let status = api.register();
    if status != 0 {
        return status;
    }
    let Some(list) = api.list() else {
        return 1;
    };
    for index in 0..api.count(&list) {
        if api
            .identifier(&list, index)
            .is_some_and(|id| owned_identifier(&id))
        {
            let status = api.enable(&list, index);
            if status != 0 {
                return status;
            }
        }
    }
    0
}
/// Return +1 NSArray with unrelated rows preserved and original two rows appended.
/// # Safety
/// sources is a live NSArray of string-keyed NSDictionary rows, borrowed for the
/// call. Caller consumes the returned +1 object once. Null on unavailable SDK or
/// invalid input. No settings or input-source changes are performed.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_input_sources_normalized(
    sources: *mut c_void,
) -> *mut c_void {
    if sources.is_null() {
        return std::ptr::null_mut();
    }
    #[cfg(target_os = "macos")]
    {
        native::normalized(sources)
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::ptr::null_mut()
    }
}
/// Repair only a supplied NSUserDefaults, preserving the original backup key.
/// Does not invoke Carbon. 1=success, 0=invalid object/unavailable SDK.
/// # Safety
/// defaults is a live NSUserDefaults, borrowed for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_input_sources_repair(defaults: *mut c_void) -> i32 {
    if defaults.is_null() {
        return 0;
    }
    #[cfg(target_os = "macos")]
    {
        native::repair(defaults)
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}
/// Original --register-input-source action: repair HIToolbox, register this
/// bundle, enable its exact base/Japanese IDs. Main thread only. Returns the
/// first Carbon OSStatus, or 1 for unavailable SDK/defaults/list/wrong thread.
#[no_mangle]
pub extern "C" fn meltype_macos_input_source_register() -> i32 {
    #[cfg(target_os = "macos")]
    unsafe {
        native::register()
    }
    #[cfg(not(target_os = "macos"))]
    {
        1
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
    use objc2_foundation::{NSArray, NSString};
    unsafe fn class(name: &std::ffi::CStr) -> &'static AnyClass {
        AnyClass::get(name).expect("Foundation class unavailable")
    }
    unsafe fn is(object: &AnyObject, name: &std::ffi::CStr) -> bool {
        msg_send![object,isKindOfClass:class(name)]
    }
    unsafe fn rows(object: &AnyObject) -> Option<Retained<NSArray<AnyObject>>> {
        if !is(object, c"NSArray") {
            return None;
        }
        let rows = Retained::retain(
            (object as *const AnyObject)
                .cast_mut()
                .cast::<NSArray<AnyObject>>(),
        )?;
        for row in rows.iter() {
            if !is(&row, c"NSDictionary") {
                return None;
            }
            let keys: Retained<NSArray<AnyObject>> = msg_send![&row, allKeys];
            if !keys.iter().all(|key| is(&key, c"NSString")) {
                return None;
            }
        }
        Some(rows)
    }
    unsafe fn field(row: &AnyObject, key: &str) -> Option<String> {
        let key = NSString::from_str(key);
        let value: Option<Retained<AnyObject>> = msg_send![row,objectForKey:&*key];
        let value = value.filter(|v| is(v, c"NSString"))?;
        Some(Retained::cast_unchecked::<NSString>(value).to_string())
    }
    unsafe fn normalize(rows: &NSArray<AnyObject>) -> Retained<AnyObject> {
        let result: Retained<AnyObject> = msg_send![class(c"NSMutableArray"), new];
        for row in rows.iter() {
            if field(&row, "Bundle ID").as_deref() == Some(IDENTIFIER)
                || field(&row, "Input Mode").is_some_and(|id| id.starts_with(IDENTIFIER))
            {
                continue;
            }
            let _: () = msg_send![&result,addObject:&*row];
        }
        for mode in [false, true] {
            let row: Retained<AnyObject> = msg_send![class(c"NSMutableDictionary"), new];
            let mut fields = vec![
                ("Bundle ID", IDENTIFIER.to_owned()),
                (
                    "InputSourceKind",
                    if mode {
                        "Input Mode"
                    } else {
                        "Keyboard Input Method"
                    }
                    .to_owned(),
                ),
            ];
            if mode {
                fields.push(("Input Mode", format!("{IDENTIFIER}.Japanese")));
            }
            for (key, value) in fields {
                let key = NSString::from_str(key);
                let value = NSString::from_str(&value);
                let _: () = msg_send![&row,setObject:&*value,forKey:&*key];
            }
            let _: () = msg_send![&result,addObject:&*row];
        }
        result
    }
    pub unsafe fn normalized(pointer: *mut c_void) -> *mut c_void {
        let Some(sources) = Retained::retain(pointer.cast::<AnyObject>()) else {
            return std::ptr::null_mut();
        };
        let Some(rows) = rows(&sources) else {
            return std::ptr::null_mut();
        };
        Retained::into_raw(normalize(&rows)).cast()
    }
    pub unsafe fn repair(pointer: *mut c_void) -> i32 {
        let Some(defaults) = Retained::retain(pointer.cast::<AnyObject>()) else {
            return 0;
        };
        if !is(&defaults, c"NSUserDefaults") {
            return 0;
        }
        let key = NSString::from_str("AppleEnabledInputSources");
        let value: Option<Retained<AnyObject>> = msg_send![&defaults,objectForKey:&*key];
        let sources = value
            .as_ref()
            .and_then(|value| rows(value))
            .unwrap_or_else(|| msg_send![class(c"NSArray"), new]);
        let repaired = normalize(&sources);
        let equal: bool = msg_send![&*sources,isEqualToArray:&*repaired];
        if !equal {
            let backup = NSString::from_str("MeltypeInputSourcesBeforeRepair");
            let _: () = msg_send![&defaults,setObject:&*sources,forKey:&*backup];
            let _: () = msg_send![&defaults,setObject:&*repaired,forKey:&*key];
            let _: bool = msg_send![&defaults, synchronize];
        }
        1
    }
    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn TISRegisterInputSource(bundle: *const c_void) -> i32;
        fn TISCreateInputSourceList(filter: *const c_void, include_all: u8) -> *const c_void;
        fn TISGetInputSourceProperty(source: *const c_void, key: *const c_void) -> *const c_void;
        fn TISEnableInputSource(source: *const c_void) -> i32;
        static kTISPropertyInputSourceID: *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(value: *const c_void);
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
    }
    struct List(*const c_void);
    impl Drop for List {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0) }
        }
    }
    struct Carbon {
        url: Retained<AnyObject>,
    }
    impl InputSources for Carbon {
        type List = List;
        fn register(&mut self) -> i32 {
            unsafe { TISRegisterInputSource(Retained::as_ptr(&self.url).cast()) }
        }
        fn list(&mut self) -> Option<List> {
            let list = unsafe { TISCreateInputSourceList(std::ptr::null(), 1) };
            (!list.is_null()).then(|| List(list))
        }
        fn count(&self, list: &List) -> usize {
            unsafe { CFArrayGetCount(list.0).max(0) as usize }
        }
        fn identifier(&self, list: &List, index: usize) -> Option<String> {
            unsafe {
                let source = CFArrayGetValueAtIndex(list.0, index as isize);
                let value = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
                value.cast::<NSString>().as_ref().map(ToString::to_string)
            }
        }
        fn enable(&mut self, list: &List, index: usize) -> i32 {
            unsafe { TISEnableInputSource(CFArrayGetValueAtIndex(list.0, index as isize)) }
        }
    }
    pub unsafe fn register() -> i32 {
        if MainThreadMarker::new().is_none() {
            return 1;
        }
        let suite = NSString::from_str("com.apple.HIToolbox");
        let defaults: Allocated<AnyObject> = msg_send![class(c"NSUserDefaults"), alloc];
        let defaults: Option<Retained<AnyObject>> = msg_send![defaults,initWithSuiteName:&*suite];
        let Some(defaults) = defaults else {
            return 1;
        };
        if repair(Retained::as_ptr(&defaults).cast_mut().cast()) == 0 {
            return 1;
        }
        let bundle: Retained<AnyObject> = msg_send![class(c"NSBundle"), mainBundle];
        let url: Retained<AnyObject> = msg_send![&bundle, bundleURL];
        register_sources(&mut Carbon { url })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    type Trace = Rc<RefCell<Vec<String>>>;
    struct List {
        trace: Trace,
        ids: Vec<Option<String>>,
    }
    impl Drop for List {
        fn drop(&mut self) {
            self.trace.borrow_mut().push("release list".into())
        }
    }
    struct Fake {
        trace: Trace,
        registration: i32,
        fail: Option<usize>,
        no_list: bool,
    }
    impl InputSources for Fake {
        type List = List;
        fn register(&mut self) -> i32 {
            self.trace.borrow_mut().push("register".into());
            self.registration
        }
        fn list(&mut self) -> Option<List> {
            self.trace.borrow_mut().push("list all".into());
            (!self.no_list).then(|| List {
                trace: self.trace.clone(),
                ids: vec![
                    None,
                    Some("other".into()),
                    Some(format!("{IDENTIFIER}.Other")),
                    Some(IDENTIFIER.into()),
                    Some(format!("{IDENTIFIER}.Japanese")),
                    Some(IDENTIFIER.into()),
                ],
            })
        }
        fn count(&self, list: &List) -> usize {
            list.ids.len()
        }
        fn identifier(&self, list: &List, index: usize) -> Option<String> {
            list.ids[index].clone()
        }
        fn enable(&mut self, _: &List, index: usize) -> i32 {
            self.trace.borrow_mut().push(format!("enable {index}"));
            if self.fail == Some(index) {
                -42
            } else {
                0
            }
        }
    }
    #[test]
    fn carbon_status_order_exact_ids_and_release() {
        for (status, fail, no_list, expected, events) in [
            (-50, None, false, -50, vec!["register"]),
            (0, None, true, 1, vec!["register", "list all"]),
            (
                0,
                None,
                false,
                0,
                vec![
                    "register",
                    "list all",
                    "enable 3",
                    "enable 4",
                    "enable 5",
                    "release list",
                ],
            ),
            (
                0,
                Some(3),
                false,
                -42,
                vec!["register", "list all", "enable 3", "release list"],
            ),
            (
                0,
                Some(4),
                false,
                -42,
                vec![
                    "register",
                    "list all",
                    "enable 3",
                    "enable 4",
                    "release list",
                ],
            ),
            (
                0,
                Some(5),
                false,
                -42,
                vec![
                    "register",
                    "list all",
                    "enable 3",
                    "enable 4",
                    "enable 5",
                    "release list",
                ],
            ),
        ] {
            let trace = Trace::default();
            let mut api = Fake {
                trace: trace.clone(),
                registration: status,
                fail,
                no_list,
            };
            assert_eq!(register_sources(&mut api), expected);
            assert_eq!(*trace.borrow(), events);
        }
    }
}
