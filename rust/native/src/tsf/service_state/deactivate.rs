// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Native shutdown state, slots and owned resource cleanup.
use super::*;
use crate::tsf::shutdown;
type Request = unsafe extern "C" fn(*mut c_void, *const c_char, u32, *mut c_void, *mut c_void);
type End = unsafe extern "C" fn(*mut c_void, u32);
type Notice = unsafe extern "C" fn(*mut c_void, i32);
struct Owner {
    state: *mut ServiceState,
    lifetime: *mut c_void,
    context: *mut c_void,
    request: Request,
    end: Option<End>,
    notice: Option<Notice>,
}
unsafe extern "C" fn read(data: *mut c_void) -> u32 {
    u32::from((*(*data.cast::<Owner>()).state).disabled)
}
unsafe extern "C" fn action(data: *mut c_void, op: u32, cookie: u32, manager: *mut c_void) {
    let owner = &*data.cast::<Owner>();
    let state = owner.state;
    match op {
        0 => (*state).deactivated = true,
        1 => meltype_tsf_service_clear_resume(state),
        2 => key_state::meltype_tsf_key_cache_clear((*state).keys),
        3 => {
            let token = (*state).abandoned;
            if !token.is_null() {
                let status =
                    abandoned::meltype_tsf_abandoned_end(token, owner.lifetime, (*state).client);
                if status.is_err() {
                    if let Some(notice) = owner.notice {
                        notice(owner.context, status.0);
                    }
                }
            }
        }
        4 => meltype_tsf_service_close(state),
        5 => meltype_tsf_service_drop_candidates(state),
        6 => {
            let _ = meltype_tsf_service_edit_sink(
                state,
                ptr::null_mut(),
                owner.lifetime,
                ptr::null_mut(),
            );
        }
        7 => meltype_tsf_service_remove_langbar(state, manager),
        8 => (*state).client = 0,
        9 => match owner.end {
            Some(end) => end(owner.context, cookie),
            None => {
                super::owned::meltype_tsf_service_end_composition(state, owner.lifetime, cookie);
            }
        },
        _ => {}
    }
}
unsafe extern "C" fn request(
    data: *mut c_void,
    text: *const c_char,
    cookie: u32,
    context: *mut c_void,
    target: *mut c_void,
) {
    let owner = &*data.cast::<Owner>();
    (owner.request)(owner.context, text, cookie, context, target);
}
/// # Safety
/// Live state and lifetime IUnknown on the TSF thread. Slots obey service COM
/// ownership. Lifetime pins state/context/code through reentry. Request applies
/// replies only to matching context/target; end uses the supplied edit cookie.
/// Callbacks never unwind or retain arguments. Delayed sessions retain only the
/// generic shutdown work/pin; their inactive gate prevents using this stack.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_deactivate(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    context: *mut c_void,
    request_callback: Option<Request>,
    end: Option<End>,
    notice: Option<Notice>,
) -> HRESULT {
    let (Some(request_callback), Some(end)) = (request_callback, end) else {
        return E_INVALIDARG;
    };
    run(
        state,
        lifetime,
        context,
        request_callback,
        Some(end),
        notice,
    )
}
/// Service shutdown with native composition ending and stale-key cleanup.
/// # Safety
/// Same lifetime/slot/callback contract as service_deactivate. Request is the only
/// remaining operation adapter. Late sessions cannot use its stack context.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_stop(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    context: *mut c_void,
    request_callback: Option<Request>,
    notice: Option<Notice>,
) -> HRESULT {
    let Some(request_callback) = request_callback else {
        return E_INVALIDARG;
    };
    run(state, lifetime, context, request_callback, None, notice)
}
unsafe fn run(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    context: *mut c_void,
    request_callback: Request,
    end: Option<End>,
    notice: Option<Notice>,
) -> HRESULT {
    if state.is_null() || lifetime.is_null() {
        return E_INVALIDARG;
    }
    let sid = CStr::from_ptr(meltype_tsf_service_sid(state)).to_string_lossy();
    let metadata = CString::new(serde_json::json!({"sid":sid}).to_string()).unwrap();
    let client = (*state).client;
    let mut owner = Owner {
        state,
        lifetime,
        context,
        request: request_callback,
        end,
        notice,
    };
    let callbacks = shutdown::Callbacks {
        owner: ptr::addr_of_mut!(owner).cast(),
        lifetime,
        epoch: ptr::addr_of_mut!((*state).focus_epoch),
        phase: ptr::addr_of_mut!((*state).shutdown_phase),
        composition: ptr::addr_of_mut!((*state).composition),
        context: ptr::addr_of_mut!((*state).composition_context),
        manager: ptr::addr_of_mut!((*state).manager),
        mode_subscription: ptr::addr_of_mut!((*state).mode_subscription),
        key_subscription: ptr::addr_of_mut!((*state).key_subscription),
        thread_subscription: ptr::addr_of_mut!((*state).thread_subscription),
        read,
        action,
        request,
    };
    shutdown::meltype_tsf_shutdown(client, metadata.as_ptr(), &callbacks)
}

type NativeNotice = unsafe extern "C" fn(*mut c_void, u32, i32);
struct NativeOwner {
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    worker: *const c_char,
    adapter: *mut c_void,
    notice: Option<NativeNotice>,
}
struct NativeReply<'a> {
    owner: &'a NativeOwner,
    context: *mut c_void,
    target: *mut c_void,
    cookie: u32,
}
unsafe extern "C" fn native_failure(data: *mut c_void) {
    let o = &*data.cast::<NativeOwner>();
    if let Some(notice) = o.notice {
        notice(o.adapter, 1, 0);
    }
}
unsafe extern "C" fn reply_failure(data: *mut c_void) {
    native_failure((*data.cast::<NativeReply>()).owner as *const NativeOwner as *mut c_void);
}
unsafe extern "C" fn native_parsed(data: *mut c_void, bytes: *const u8, length: u32) -> i32 {
    let r = &*data.cast::<NativeReply>();
    let o = r.owner;
    if !crate::json_bridge::valid(std::slice::from_raw_parts(bytes, length as usize)) {
        return 0;
    }
    // Close replies are validated and discarded; shutdown only applies a commit
    // while its original composition/context pair is still current.
    if r.context.is_null()
        || (*o.state).composition_context != r.context
        || (*o.state).composition != r.target
    {
        return 1;
    }
    i32::from(
        super::apply::meltype_tsf_service_apply_reply(
            o.state,
            o.lifetime,
            o.sink,
            o.worker,
            r.context,
            r.cookie,
            bytes,
            length,
            o as *const NativeOwner as *mut c_void,
            Some(native_failure),
        ) >= 0,
    )
}
unsafe extern "C" fn native_request(
    data: *mut c_void,
    text: *const c_char,
    cookie: u32,
    context: *mut c_void,
    target: *mut c_void,
) {
    let o = &*data.cast::<NativeOwner>();
    let bytes = CStr::from_ptr(text).to_bytes();
    let Ok(length) = u32::try_from(bytes.len()) else {
        return;
    };
    let mut reply = NativeReply {
        owner: o,
        context,
        target,
        cookie,
    };
    super::exchange::meltype_tsf_service_exchange(
        o.state,
        o.worker,
        o.lifetime,
        bytes.as_ptr(),
        length,
        800,
        0,
        ptr::addr_of_mut!(reply).cast(),
        Some(native_parsed),
        Some(reply_failure),
    );
}
unsafe extern "C" fn native_cleanup(data: *mut c_void, status: i32) {
    let o = &*data.cast::<NativeOwner>();
    if let Some(notice) = o.notice {
        notice(o.adapter, 0, status);
    }
}
/// Complete service shutdown, including native exchange and reply application.
/// # Safety
/// Live service/root/composition sink on its TSF thread. Worker is trusted
/// absolute UTF-8. Root pins state/notice code through reentrant cleanup. Notice
/// op0=abandoned cleanup HRESULT,op1=invalid reply, never unwinds. Deferred SYNC
/// sessions are invalidated and cannot use these stack adapters after return.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_shutdown(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    worker: *const c_char,
    adapter: *mut c_void,
    notice: Option<NativeNotice>,
) -> HRESULT {
    if worker.is_null() {
        return E_INVALIDARG;
    }
    let mut owner = NativeOwner {
        state,
        lifetime,
        sink,
        worker,
        adapter,
        notice,
    };
    run(
        state,
        lifetime,
        ptr::addr_of_mut!(owner).cast(),
        native_request,
        None,
        Some(native_cleanup),
    )
}
