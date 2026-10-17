// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Compartment notifications using native service mode and language bar.
use super::*;
use crate::tsf::open_change;
use windows::core::GUID;
type Commit = unsafe extern "C" fn(*mut c_void);
type Notice = unsafe extern "C" fn(*mut c_void, i32);
struct Owner {
    state: *mut ServiceState,
    context: *mut c_void,
    commit: Commit,
    notice: Option<Notice>,
}
unsafe extern "C" fn current(data: *mut c_void, manager: *mut *mut c_void) -> u32 {
    let state = (*data.cast::<Owner>()).state;
    manager.write((*state).manager);
    u32::from((*state).open)
        | (u32::from(!(*state).composition.is_null()) << 1)
        | (u32::from((*state).deactivated) << 2)
}
unsafe extern "C" fn update(data: *mut c_void, op: u32, value: i32) {
    let owner = &*data.cast::<Owner>();
    let state = owner.state;
    match op {
        0 => {
            (*state).open = value != 0;
            (*state).direct_word = false;
        }
        1 => (owner.commit)(owner.context),
        2 => meltype_tsf_service_update_langbar(state),
        3 => {
            if let Some(notice) = owner.notice {
                notice(owner.context, i32::from((*state).open));
            }
        }
        _ => {}
    }
}
/// Read/publish OS mode, notify, optionally commit without preserving caret and
/// refresh the owned language bar. Reentrant mode/manager changes supersede work.
/// # Safety
/// Live state, lifetime IUnknown and readable GUID on the TSF thread. Lifetime
/// pins state/context/code across callbacks. Commit and optional notice never
/// unwind or retain stack arguments. Manager slots follow service ownership.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_opened(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    guid: *const GUID,
    context: *mut c_void,
    commit: Option<Commit>,
    notice: Option<Notice>,
) -> HRESULT {
    let Some(commit) = commit else {
        return E_INVALIDARG;
    };
    if state.is_null() || lifetime.is_null() {
        return E_INVALIDARG;
    }
    let mut owner = Owner {
        state,
        context,
        commit,
        notice,
    };
    let callbacks = open_change::Callbacks {
        owner: ptr::addr_of_mut!(owner).cast(),
        lifetime,
        epoch: ptr::addr_of_mut!((*state).mode_epoch),
        phase: ptr::addr_of_mut!((*state).shutdown_phase),
        current,
        update,
    };
    open_change::meltype_tsf_open_changed(guid, &callbacks)
}
