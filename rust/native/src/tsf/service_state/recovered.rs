// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Host termination/recovery against service state, without reply reinsertion.
use super::*;
use crate::tsf::{abandon_control, recovery_control, termination};
type Notice = unsafe extern "C" fn(*mut c_void);
struct Owner {
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    context: *mut c_void,
    notice: Option<Notice>,
}
unsafe fn metadata(state: *mut ServiceState) -> CString {
    let sid = CStr::from_ptr(meltype_tsf_service_sid(state)).to_string_lossy();
    CString::new(serde_json::json!({"sid":sid}).to_string()).unwrap()
}
unsafe extern "C" fn valid(_: *mut c_void, data: *const u8, length: u32) -> i32 {
    i32::from(crate::json_bridge::valid(std::slice::from_raw_parts(
        data,
        length as usize,
    )))
}
unsafe extern "C" fn failure(data: *mut c_void) {
    let owner = &*data.cast::<Owner>();
    if let Some(notice) = owner.notice {
        notice(owner.context);
    }
}
unsafe extern "C" fn request(data: *mut c_void, text: *const c_char) {
    let owner = &*data.cast::<Owner>();
    let bytes = CStr::from_ptr(text).to_bytes();
    if let Ok(length) = u32::try_from(bytes.len()) {
        super::exchange::meltype_tsf_service_exchange(
            owner.state,
            owner.worker,
            owner.lifetime,
            bytes.as_ptr(),
            length,
            800,
            0,
            data,
            Some(valid),
            Some(failure),
        );
    }
}
unsafe extern "C" fn flags(data: *mut c_void) -> u32 {
    let state = (*data.cast::<Owner>()).state;
    u32::from((*state).disabled)
        | (u32::from((*state).deactivated) << 1)
        | (u32::from((*state).last_converting) << 2)
}
unsafe extern "C" fn terminated_update(data: *mut c_void, op: u32) {
    let state = (*data.cast::<Owner>()).state;
    match op {
        0 => meltype_tsf_service_hide_candidates(state),
        1 => {
            key_state::meltype_tsf_key_stale_clear((*state).keys);
            (*state).last_converting = false;
        }
        2 => {
            (*state).direct_word = false;
            (*state).caret_moved = true;
        }
        _ => {}
    }
}
unsafe extern "C" fn edit_context(data: *mut c_void) -> *mut c_void {
    (*(*data.cast::<Owner>()).state).edit_context
}
unsafe extern "C" fn recovered_update(data: *mut c_void, op: u32, value: i32) {
    let state = (*data.cast::<Owner>()).state;
    if op == 0 {
        (*state).own_edit = value != 0;
    } else if op == 1 {
        (*state).direct_word = false;
        (*state).caret_moved = true;
    }
}
/// # Safety
/// Live service/lifetime and borrowed composition on the TSF thread, valid edit
/// cookie. Lifetime pins state/context/notice code. Worker is a trusted absolute
/// UTF-8 executable path. Optional parse-failure notice never unwinds. Requests
/// validate replies using the upstream-compatible parser and never apply them.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_terminated(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    cookie: u32,
    composition: *mut c_void,
    context: *mut c_void,
    notice: Option<Notice>,
) -> HRESULT {
    if state.is_null() || lifetime.is_null() || worker.is_null() {
        return E_INVALIDARG;
    }
    let metadata = metadata(state);
    let mut owner = Owner {
        state,
        lifetime,
        worker,
        context,
        notice,
    };
    let callbacks = termination::Callbacks {
        owner: ptr::addr_of_mut!(owner).cast(),
        lifetime,
        composition: ptr::addr_of_mut!((*state).composition),
        context: ptr::addr_of_mut!((*state).composition_context),
        resume: ptr::addr_of_mut!((*state).resume),
        abandoned: (*state).abandoned,
        read: flags,
        update: terminated_update,
        request,
    };
    termination::meltype_tsf_composition_terminated(
        cookie,
        composition,
        metadata.as_ptr(),
        &callbacks,
    )
}
/// # Safety
/// Same service/request contract. Context/sink are borrowed TSF interfaces (null
/// context means fallback commit). UI callbacks and slots survive while pinned.
/// A delayed edit session is invalidated before this stack adapter returns.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_resume(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    sink: *mut c_void,
    context: *mut c_void,
    adapter: *mut c_void,
    notice: Option<Notice>,
) -> i32 {
    if state.is_null() || lifetime.is_null() || worker.is_null() {
        return -1;
    }
    let metadata = metadata(state);
    let mut owner = Owner {
        state,
        lifetime,
        worker,
        context: adapter,
        notice,
    };
    let callbacks = recovery_control::Callbacks {
        owner: ptr::addr_of_mut!(owner).cast(),
        lifetime,
        composition: ptr::addr_of_mut!((*state).composition),
        context: ptr::addr_of_mut!((*state).composition_context),
        resume: ptr::addr_of_mut!((*state).resume),
        sink,
        edit_context,
        update: recovered_update,
        request,
    };
    recovery_control::meltype_tsf_resume_or_commit(
        context,
        (*state).client,
        (*state).atom_input,
        metadata.as_ptr(),
        &callbacks,
    )
}

unsafe extern "C" fn abandoned_update(data: *mut c_void, op: u32) {
    let state = (*data.cast::<Owner>()).state;
    match op {
        0 => key_state::meltype_tsf_key_stale_clear((*state).keys),
        1 => meltype_tsf_service_hide_candidates(state),
        2 => {
            (*state).direct_word = false;
            (*state).caret_moved = true;
        }
        _ => {}
    }
}
/// Detach uneditable old input, queue cleanup and send a reply-discarding commit.
/// # Safety
/// Same service/lifetime/request contract as service_terminated. Owned slots
/// transfer their refs into the abandoned queue; replacement input is preserved.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_abandon(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    context: *mut c_void,
    notice: Option<Notice>,
) -> i32 {
    if state.is_null() || lifetime.is_null() || worker.is_null() {
        return -1;
    }
    let metadata = metadata(state);
    let mut owner = Owner {
        state,
        lifetime,
        worker,
        context,
        notice,
    };
    let callbacks = abandon_control::Callbacks {
        owner: ptr::addr_of_mut!(owner).cast(),
        lifetime,
        composition: ptr::addr_of_mut!((*state).composition),
        context: ptr::addr_of_mut!((*state).composition_context),
        abandoned: ptr::addr_of_mut!((*state).abandoned),
        update: abandoned_update,
        request,
    };
    abandon_control::meltype_tsf_abandon_composition((*state).client, metadata.as_ptr(), &callbacks)
}
