// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Mode switching and privacy-gated code-line requests on the TSF thread.
use super::{edit_session, fields, key_state, ranges};
use serde_json::{json, Value};
use std::{
    cell::{Cell, RefCell},
    ffi::{c_char, c_void, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    rc::Rc,
};
use windows::{
    core::{IUnknown, Interface, HRESULT},
    Win32::{Foundation::*, UI::TextServices::*},
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Callbacks {
    owner: *mut c_void,
    lifetime: *mut c_void,
    read: unsafe extern "C" fn(*mut c_void) -> u32,
    ready: unsafe extern "C" fn(*mut c_void) -> i32,
    secret: unsafe extern "C" fn(*mut c_void, u32, *mut c_void) -> i32,
    set_open: unsafe extern "C" fn(*mut c_void, i32),
    request: unsafe extern "C" fn(*mut c_void, *const c_char) -> i32,
    notice: unsafe extern "C" fn(*mut c_void, i32),
}
#[derive(Default)]
struct ReadResult {
    secret: bool,
    before: String,
    executed: bool,
}
struct Read {
    context: ITfContext,
    _owner: IUnknown,
    callbacks: Callbacks,
    result: Rc<RefCell<ReadResult>>,
    active: Rc<Cell<bool>>,
}
unsafe extern "C" fn receive_before(
    owner: *mut c_void,
    text: *const u16,
    length: u32,
    _: *const u16,
    _: u32,
) {
    let out = &mut *owner.cast::<String>();
    *out = String::from_utf16_lossy(std::slice::from_raw_parts(text, length as usize));
}
unsafe extern "C" fn read(owner: *mut c_void, cookie: u32) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let r = &*owner.cast::<Read>();
        // Even a misbehaving host retaining a SYNC session cannot read text later.
        if !r.active.get() {
            return E_ABORT;
        }
        let secret = (r.callbacks.secret)(r.callbacks.owner, cookie, r.context.as_raw()) != 0;
        let mut before = String::new();
        if !secret {
            ranges::meltype_tsf_read_surrounding(
                r.context.as_raw(),
                cookie,
                (&mut before as *mut String).cast(),
                receive_before,
            );
        }
        *r.result.borrow_mut() = ReadResult {
            secret,
            before,
            executed: true,
        };
        S_OK
    }))
    .unwrap_or(E_FAIL)
}
unsafe extern "C" fn destroy(owner: *mut c_void) {
    drop(Box::from_raw(owner.cast::<Read>()));
}

unsafe fn code_line(
    context: Option<&ITfContext>,
    token: *mut c_void,
    client: u32,
    metadata: &Value,
    c: Callbacks,
    owner: &IUnknown,
    japanese: bool,
) -> bool {
    let Some(context) = context else { return false };
    if fields::meltype_tsf_context_disabled(context.as_raw()) != 0 || (c.ready)(c.owner) == 0 {
        return false;
    }
    let mut request = json!({"op":"toggle", "sid":metadata["sid"], "process":metadata["process"], "japanese":japanese});
    let mut passed = key_state::Passed::default();
    if key_state::meltype_tsf_key_passed(token, 0, 0, &mut passed) != 0 {
        request["passed"] = json!(passed.key);
        if passed.control != 0 {
            request["ctrl"] = json!(true);
        }
    }
    let result = Rc::new(RefCell::new(ReadResult::default()));
    let active = Rc::new(Cell::new(true));
    let work = Box::new(Read {
        context: context.clone(),
        _owner: owner.clone(),
        callbacks: c,
        result: result.clone(),
        active: active.clone(),
    });
    let status = edit_session::meltype_tsf_request_edit_session(
        context.as_raw(),
        client,
        (TF_ES_SYNC | TF_ES_READ).0,
        Box::into_raw(work).cast(),
        Some(read),
        destroy,
    );
    active.set(false);
    let result = result.borrow();
    if result.secret {
        return false;
    }
    // Refused read locks preserve the original fallback to the tracked code line.
    if status.is_ok() && result.executed {
        request["before"] = json!(result.before);
    }
    let Ok(request) = CString::new(request.to_string()) else {
        return false;
    };
    let done = (c.request)(c.owner, request.as_ptr()) != 0;
    if done {
        (c.notice)(c.owner, i32::from(japanese));
    }
    done
}
unsafe fn focus(manager: *mut c_void) -> Option<ITfContext> {
    let manager = ITfThreadMgr::from_raw_borrowed(&manager)?.clone();
    let mut raw = ptr::null_mut();
    let status = (Interface::vtable(&manager).GetFocus)(manager.as_raw(), &mut raw);
    let document = if raw.is_null() {
        None
    } else {
        Some(ITfDocumentMgr::from_raw(raw))
    };
    if status.is_err() {
        return None;
    }
    let document = document?;
    raw = ptr::null_mut();
    let status = (Interface::vtable(&document).GetTop)(document.as_raw(), &mut raw);
    let context = if raw.is_null() {
        None
    } else {
        Some(ITfContext::from_raw(raw))
    };
    if status.is_err() {
        None
    } else {
        context
    }
}
enum Action {
    Toggle,
    CodeLine(bool),
}
unsafe fn dispatch(
    context: *mut c_void,
    manager: *mut c_void,
    token: *mut c_void,
    client: u32,
    metadata: *const c_char,
    callbacks: *const Callbacks,
    action: Action,
) -> bool {
    let Some(c) = callbacks.as_ref().copied() else {
        return false;
    };
    let Some(owner) = IUnknown::from_raw_borrowed(&c.lifetime).cloned() else {
        return false;
    };
    if metadata.is_null() {
        return false;
    }
    let Ok(metadata) = serde_json::from_slice::<Value>(CStr::from_ptr(metadata).to_bytes()) else {
        return false;
    };
    if !metadata["sid"].is_string() || !metadata["process"].is_string() {
        return false;
    }
    let context = ITfContext::from_raw_borrowed(&context)
        .cloned()
        .or_else(|| {
            if matches!(action, Action::Toggle) {
                focus(manager)
            } else {
                None
            }
        });
    if let Action::CodeLine(japanese) = action {
        return code_line(
            context.as_ref(),
            token,
            client,
            &metadata,
            c,
            &owner,
            japanese,
        );
    }
    let flags = (c.read)(c.owner);
    if flags & 1 != 0 && flags & 4 == 0 {
        if flags & 2 == 0
            || !code_line(context.as_ref(), token, client, &metadata, c, &owner, false)
        {
            (c.set_open)(c.owner, 0);
        }
        return true;
    }
    if flags & 1 == 0 {
        (c.set_open)(c.owner, 1);
    }
    let flags = (c.read)(c.owner); // SetOpen can synchronously notify and change the code-line state.
    if flags & 2 != 0 && flags & 4 != 0 {
        code_line(context.as_ref(), token, client, &metadata, c, &owner, true);
    }
    true
}
/// # Safety
/// Borrowed TSF interfaces on their creating thread; live callback function
/// pointers which never throw. read bits: open=1, code application=2, code English=4.
/// request parses/applies the core reply and returns active && consumed.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_toggle_mode(
    context: *mut c_void,
    manager: *mut c_void,
    token: *mut c_void,
    client: u32,
    metadata: *const c_char,
    callbacks: *const Callbacks,
) -> i32 {
    i32::from(
        catch_unwind(AssertUnwindSafe(|| {
            dispatch(
                context,
                manager,
                token,
                client,
                metadata,
                callbacks,
                Action::Toggle,
            )
        }))
        .unwrap_or(false),
    )
}
/// # Safety
/// Same ABI contract as meltype_tsf_toggle_mode; no focus fallback for null context.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_code_line(
    context: *mut c_void,
    token: *mut c_void,
    client: u32,
    metadata: *const c_char,
    callbacks: *const Callbacks,
    japanese: i32,
) -> i32 {
    i32::from(
        catch_unwind(AssertUnwindSafe(|| {
            dispatch(
                context,
                ptr::null_mut(),
                token,
                client,
                metadata,
                callbacks,
                Action::CodeLine(japanese != 0),
            )
        }))
        .unwrap_or(false),
    )
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct QueryCallbacks {
    owner: *mut c_void,
    lifetime: *mut c_void,
    read: unsafe extern "C" fn(*mut c_void) -> u32,
    secret: unsafe extern "C" fn(*mut c_void, u32, *mut c_void) -> i32,
    request: unsafe extern "C" fn(*mut c_void, *const c_char, u32, i32) -> i32,
}
/// Query mode during an existing read edit session; no new lock or hello.
/// # Safety
/// Borrowed TSF context, valid read cookie, JSON {sid:string,process:string} and
/// non-null callback functions on their TSF thread. Callbacks never throw.
/// read: code application=2, server active=8. request applies a successful reply
/// and receives timeout=100ms, optional=1. Pins context and owner until return.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_query_mode(
    context: *mut c_void,
    cookie: u32,
    token: *mut c_void,
    metadata: *const c_char,
    callbacks: *const QueryCallbacks,
    selection_changed: i32,
) -> i32 {
    i32::from(catch_unwind(AssertUnwindSafe(|| {
        let Some(c) = callbacks.as_ref().copied() else { return false; };
        let Some(_owner) = IUnknown::from_raw_borrowed(&c.lifetime).cloned() else { return false; };
        let flags = (c.read)(c.owner);
        if flags & (2 | 8) != (2 | 8) { return false; }
        let Some(context) = ITfContext::from_raw_borrowed(&context).cloned() else { return false; };
        if fields::meltype_tsf_context_disabled(context.as_raw()) != 0
            || (c.secret)(c.owner, cookie, context.as_raw()) != 0 { return false; }
        if metadata.is_null() { return false; }
        let Ok(metadata) = serde_json::from_slice::<Value>(CStr::from_ptr(metadata).to_bytes()) else { return false; };
        if !metadata["sid"].is_string() || !metadata["process"].is_string() { return false; }
        let mut before = String::new();
        ranges::meltype_tsf_read_surrounding(context.as_raw(), cookie, (&mut before as *mut String).cast(), receive_before);
        let mut passed = key_state::Passed::default();
        key_state::meltype_tsf_key_passed(token, 1, selection_changed, &mut passed);
        let mut request = json!({"op":"mode", "sid":metadata["sid"], "process":metadata["process"], "passed":passed.key,"before":before});
        if passed.control != 0 { request["ctrl"] = json!(true); }
        let Ok(request) = CString::new(request.to_string()) else { return false; };
        (c.request)(c.owner, request.as_ptr(), 100, 1) != 0
    })).unwrap_or(false))
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ApplyCallbacks {
    owner: *mut c_void,
    lifetime: *mut c_void,
    read: unsafe extern "C" fn(*mut c_void) -> u32,
    write: unsafe extern "C" fn(*mut c_void, u32),
    notify: unsafe extern "C" fn(*mut c_void, i32),
}
/// Normalize the core mode response, publish both fields before notifying UI.
/// # Safety
/// On the TSF thread with non-null function pointers and a borrowed owner
/// IUnknown. Callbacks never throw. reply bits: active=1,code=2,English=4;
/// read/write use code application=2,code English=4. write publishes without
/// notifications; notify runs only when code English changes, with owner pinned.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_apply_mode(
    reply: u32,
    callbacks: *const ApplyCallbacks,
) -> i32 {
    i32::from(
        catch_unwind(AssertUnwindSafe(|| {
            let Some(c) = callbacks.as_ref().copied() else {
                return false;
            };
            let Some(_owner) = IUnknown::from_raw_borrowed(&c.lifetime).cloned() else {
                return false;
            };
            let previous = (c.read)(c.owner);
            let code = reply & 3 == 3;
            let english = code && reply & 4 != 0;
            (c.write)(
                c.owner,
                if code { 2 } else { 0 } | if english { 4 } else { 0 },
            );
            if (previous & 4 != 0) != english {
                (c.notify)(c.owner, i32::from(english));
            }
            true
        }))
        .unwrap_or(false),
    )
}
