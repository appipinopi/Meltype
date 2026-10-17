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
    end: End,
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
        9 => (owner.end)(owner.context, cookie),
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
