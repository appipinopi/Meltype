// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Native service focus actions; only composition operations still use adapters.
use super::*;
use crate::tsf::focus;
type Commit = unsafe extern "C" fn(*mut c_void, i32) -> i32;
type Resume = unsafe extern "C" fn(*mut c_void);
type Notice = unsafe extern "C" fn(*mut c_void, u32, i32);
struct Owner {
    state: *mut ServiceState,
    worker: *const c_char,
    lifetime: *mut c_void,
    sink: *mut c_void,
    context: *mut c_void,
    commit: Commit,
    resume: Resume,
    notice: Option<Notice>,
}
unsafe extern "C" fn read(data: *mut c_void) -> u32 {
    let state = (*data.cast::<Owner>()).state;
    u32::from(!(*state).composition.is_null())
        | (u32::from((*state).disabled) << 1)
        | (u32::from((*state).deactivated || (*state).shutdown_phase != 0) << 2)
}
unsafe extern "C" fn commit(data: *mut c_void, mode: i32) -> i32 {
    let owner = &*data.cast::<Owner>();
    (owner.commit)(owner.context, mode)
}
unsafe extern "C" fn action(data: *mut c_void, op: u32, document: *mut c_void) {
    let owner = &*data.cast::<Owner>();
    let state = owner.state;
    match op {
        0 => (owner.resume)(owner.context),
        1 => {
            (*state).direct_word = false;
            (*state).caret_moved = true;
        }
        2 => key_state::meltype_tsf_key_cache_clear((*state).keys),
        3 => meltype_tsf_service_hide_candidates(state),
        4 => {
            let _ = meltype_tsf_service_edit_sink(state, document, owner.lifetime, owner.sink);
        }
        5 => {
            super::core::meltype_tsf_service_core(
                state,
                owner.worker,
                owner.lifetime,
                owner.context,
                owner.notice,
                None,
                1,
            );
        }
        _ => {}
    }
}
/// event -1=document focus, 0=foreground lost, 1=foreground gained. Remaining
/// composition callbacks preserve the existing sync/async keep-caret behavior.
/// # Safety
/// Live state, borrowed lifetime IUnknown and optional edit sink on the TSF
/// thread. Lifetime pins state/context/code through the call. Document is a
/// borrowed ITfDocumentMgr (null is allowed). Worker is a trusted absolute UTF-8
/// executable path. Callbacks never unwind or retain arguments.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_focus(
    state: *mut ServiceState,
    worker: *const c_char,
    lifetime: *mut c_void,
    sink: *mut c_void,
    document: *mut c_void,
    event: i32,
    context: *mut c_void,
    commit_callback: Option<Commit>,
    resume: Option<Resume>,
    notice: Option<Notice>,
) -> HRESULT {
    let (Some(commit_callback), Some(resume)) = (commit_callback, resume) else {
        return E_INVALIDARG;
    };
    if state.is_null() || worker.is_null() || lifetime.is_null() || !(-1..=1).contains(&event) {
        return E_INVALIDARG;
    }
    if event == -1 && !document.is_null() && sink.is_null() {
        return E_INVALIDARG;
    }
    let mut owner = Owner {
        state,
        worker,
        lifetime,
        sink,
        context,
        commit: commit_callback,
        resume,
        notice,
    };
    let callbacks = focus::Callbacks {
        owner: ptr::addr_of_mut!(owner).cast(),
        lifetime,
        epoch: ptr::addr_of_mut!((*state).focus_epoch),
        read,
        commit,
        action,
    };
    if event == -1 {
        focus::meltype_tsf_document_focus(document, &callbacks)
    } else {
        let keys = (*state).keys;
        focus::meltype_tsf_foreground_focus(event, keys, &callbacks)
    }
}
