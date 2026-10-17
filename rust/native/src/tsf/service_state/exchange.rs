// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Service-owned request mediation. Only the temporary reply value adapter is external.
use super::*;
use crate::tsf::core_link;
use std::slice;

type Reply = unsafe extern "C" fn(*mut c_void, *const u8, u32) -> i32;
type Notice = unsafe extern "C" fn(*mut c_void);
struct Owner {
    state: *mut ServiceState,
    worker: *const c_char,
    context: *mut c_void,
    reply: Reply,
    notice: Option<Notice>,
    response: Option<String>,
}
unsafe extern "C" fn read(data: *mut c_void) -> u32 {
    let state = (*data.cast::<Owner>()).state;
    (u32::from((*state).deactivated) << 2) | (u32::from((*state).own_process) << 3)
}
unsafe extern "C" fn transact(
    data: *mut c_void,
    input: *const u8,
    length: u32,
    timeout: u32,
    optional: i32,
    response: *mut *const u8,
    count: *mut u32,
) -> i32 {
    let owner = &mut *data.cast::<Owner>();
    let bytes = if length == 0 {
        &[]
    } else {
        slice::from_raw_parts(input, length as usize)
    };
    let Ok(request) = CString::new(bytes) else {
        return 0;
    };
    owner.response = crate::take(meltype_tsf_service_request(
        owner.state,
        owner.worker,
        request.as_ptr(),
        timeout,
        optional,
    ));
    let Some(reply) = owner.response.as_ref() else {
        return 0;
    };
    let Ok(length) = u32::try_from(reply.len()) else {
        return 0;
    };
    response.write(reply.as_ptr());
    count.write(length);
    1
}
unsafe extern "C" fn parse(data: *mut c_void, bytes: *const u8, count: u32) -> i32 {
    let owner = &*data.cast::<Owner>();
    (owner.reply)(owner.context, bytes, count)
}
unsafe extern "C" fn notice(data: *mut c_void) {
    let owner = &*data.cast::<Owner>();
    if let Some(callback) = owner.notice {
        callback(owner.context);
    }
}
/// Request flags, thread metadata and owned transport are handled in Rust. Reply
/// bytes are borrowed only during the synchronous value-adapter callback.
/// # Safety
/// State and lifetime are live on the TSF thread. Lifetime pins state/context and
/// callback code through reentry. Input is readable for length bytes; null only
/// for zero length. Worker is a trusted absolute UTF-8 executable path. Callbacks
/// never unwind or retain bytes. No concurrent requests use this service.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_exchange(
    state: *mut ServiceState,
    worker: *const c_char,
    lifetime: *mut c_void,
    input: *const u8,
    length: u32,
    timeout: u32,
    optional: i32,
    context: *mut c_void,
    reply: Option<Reply>,
    failure: Option<Notice>,
) -> i32 {
    let Some(reply) = reply else {
        return 0;
    };
    if state.is_null() || worker.is_null() || lifetime.is_null() {
        return 0;
    }
    let mut owner = Owner {
        state,
        worker,
        context,
        reply,
        notice: failure,
        response: None,
    };
    let callbacks = core_link::RequestCallbacks {
        owner: ptr::addr_of_mut!(owner).cast(),
        lifetime,
        epoch: ptr::addr_of!((*state).focus_epoch),
        read,
        transact,
        parse,
        notice,
    };
    core_link::meltype_tsf_core_request(input, length, timeout, optional, &callbacks)
}
