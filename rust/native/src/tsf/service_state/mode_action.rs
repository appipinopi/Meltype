// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Complete service mode actions, including compatible reply parsing and state.
use super::*;
use crate::tsf::{mode, process};
type Notice = unsafe extern "C" fn(*mut c_void, u32, i32);
struct Owner {
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    context: *mut c_void,
    notice: Option<Notice>,
    reply: u32,
}
unsafe extern "C" fn relay(data: *mut c_void, op: u32, value: i32) {
    let owner = &*data.cast::<Owner>();
    if let Some(notice) = owner.notice {
        notice(owner.context, op, value);
    }
}
unsafe extern "C" fn read(data: *mut c_void) -> u32 {
    meltype_tsf_service_mode((*data.cast::<Owner>()).state)
}
unsafe extern "C" fn ready(data: *mut c_void) -> i32 {
    let owner = &*data.cast::<Owner>();
    super::core::meltype_tsf_service_core(
        owner.state,
        owner.worker,
        owner.lifetime,
        data,
        Some(relay),
        None,
        0,
    )
}
unsafe extern "C" fn secret(data: *mut c_void, cookie: u32, context: *mut c_void) -> i32 {
    let owner = &*data.cast::<Owner>();
    super::input::meltype_tsf_service_input_scope(
        owner.state,
        owner.lifetime,
        context,
        cookie,
        data,
        Some(relay),
    )
}
unsafe extern "C" fn set_open(data: *mut c_void, value: i32) {
    meltype_tsf_service_write_open((*data.cast::<Owner>()).state, value);
}
unsafe extern "C" fn applied(data: *mut c_void, english: i32) {
    relay(data, 2, english);
}
unsafe extern "C" fn parsed(data: *mut c_void, bytes: *const u8, length: u32) -> i32 {
    let Some(flags) =
        crate::json_bridge::mode_reply(std::slice::from_raw_parts(bytes, length as usize))
    else {
        return 0;
    };
    let owner = data.cast::<Owner>();
    (*owner).reply = flags;
    meltype_tsf_service_apply_mode(
        (*owner).state,
        flags & 7,
        (*owner).lifetime,
        data,
        Some(applied),
    );
    1
}
unsafe extern "C" fn failure(data: *mut c_void) {
    relay(data, 1, 0);
}
unsafe extern "C" fn requested(
    data: *mut c_void,
    text: *const c_char,
    timeout: u32,
    optional: i32,
) -> i32 {
    let owner = data.cast::<Owner>();
    (*owner).reply = 0;
    let bytes = CStr::from_ptr(text).to_bytes();
    let Ok(length) = u32::try_from(bytes.len()) else {
        return 0;
    };
    super::exchange::meltype_tsf_service_exchange(
        (*owner).state,
        (*owner).worker,
        (*owner).lifetime,
        bytes.as_ptr(),
        length,
        timeout,
        optional,
        data,
        Some(parsed),
        Some(failure),
    )
}
unsafe extern "C" fn request(data: *mut c_void, text: *const c_char) -> i32 {
    let ok = requested(data, text, 800, 0);
    i32::from(ok != 0 && (*data.cast::<Owner>()).reply & 9 == 9)
}
unsafe extern "C" fn changed(data: *mut c_void, japanese: i32) {
    relay(data, 3, japanese);
}
/// op0=toggle,op1=set code line(value Japanese),op2=query(cookie,value selection
/// changed). Notice 0=hello active,1=invalid JSON,2=English display,3=code line
/// Japanese,4=scope source,5=unreadable reason. Query keeps 100ms optional policy.
/// # Safety
/// Live service/lifetime/context on TSF thread with valid query cookie. Null
/// context permits toggle focus lookup. Worker is a trusted absolute UTF-8 path.
/// Lifetime pins state/notice code; optional notice never unwinds. Stack adapters
/// are disabled before delayed edit sessions can use them.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_mode_action(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    context: *mut c_void,
    cookie: u32,
    op: u32,
    value: i32,
    adapter: *mut c_void,
    notice: Option<Notice>,
) -> i32 {
    if state.is_null() || lifetime.is_null() || worker.is_null() || op > 2 {
        return 0;
    }
    let sid = CStr::from_ptr(meltype_tsf_service_sid(state)).to_string_lossy();
    let process = String::from_utf16_lossy(&process::name());
    let metadata =
        CString::new(serde_json::json!({"sid":sid,"process":process}).to_string()).unwrap();
    let mut owner = Owner {
        state,
        lifetime,
        worker,
        context: adapter,
        notice,
        reply: 0,
    };
    let data = ptr::addr_of_mut!(owner).cast();
    if op == 2 {
        let callbacks = mode::QueryCallbacks {
            owner: data,
            lifetime,
            read,
            secret,
            request: requested,
        };
        mode::meltype_tsf_query_mode(
            context,
            cookie,
            (*state).keys,
            metadata.as_ptr(),
            &callbacks,
            value,
        )
    } else {
        let callbacks = mode::Callbacks {
            owner: data,
            lifetime,
            read,
            ready,
            secret,
            set_open,
            request,
            notice: changed,
        };
        if op == 0 {
            mode::meltype_tsf_toggle_mode(
                context,
                (*state).manager,
                (*state).keys,
                (*state).client,
                metadata.as_ptr(),
                &callbacks,
            )
        } else {
            mode::meltype_tsf_code_line(
                context,
                (*state).keys,
                (*state).client,
                metadata.as_ptr(),
                &callbacks,
                value,
            )
        }
    }
}
