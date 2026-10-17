// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Activation using native service slots and owned UI resources.
use super::*;
use crate::tsf::startup;
type Select = unsafe extern "C" fn(*mut c_void, i32, i32);
type Read = unsafe extern "C" fn(*mut c_void) -> i32;
type Toggle = unsafe extern "C" fn(*mut c_void);
type Notice = unsafe extern "C" fn(*mut c_void, u32, i32);
type Category = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
struct Owner {
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    edit_sink: *mut c_void,
    context: *mut c_void,
    select: Select,
    read: Read,
    toggle: Toggle,
    notice: Option<Notice>,
    category: Option<Category>,
}
unsafe extern "C" fn initialize(
    data: *mut c_void,
    client: u32,
    disabled: i32,
    own: i32,
    sid: *const c_char,
) {
    let owner = &*data.cast::<Owner>();
    meltype_tsf_service_initialize(owner.state, client, disabled, own, sid);
    if let Some(notice) = owner.notice {
        notice(owner.context, 0, disabled);
    }
}
unsafe extern "C" fn core_notice(data: *mut c_void, op: u32, value: i32) {
    let owner = &*data.cast::<Owner>();
    if let Some(notice) = owner.notice {
        notice(owner.context, op + 1, value);
    }
}
unsafe extern "C" fn category(data: *mut c_void) -> *mut c_void {
    let owner = &*data.cast::<Owner>();
    owner.category.unwrap()(owner.context)
}
unsafe extern "C" fn action(
    data: *mut c_void,
    op: u32,
    value: i32,
    document: *mut c_void,
) -> *mut c_void {
    let owner = &*data.cast::<Owner>();
    let state = owner.state;
    match op {
        0 => (*state).open = value != 0,
        1 => {
            meltype_tsf_service_create_candidates(state, owner.context, Some(owner.select));
        }
        2 => {
            return meltype_tsf_service_create_langbar(
                state,
                owner.context,
                Some(owner.read),
                Some(owner.toggle),
            )
        }
        3 => {
            let _ = meltype_tsf_service_edit_sink(state, document, owner.lifetime, owner.edit_sink);
        }
        4 => {
            super::core::meltype_tsf_service_core(
                state,
                owner.worker,
                owner.lifetime,
                data,
                Some(core_notice),
                None,
                1,
            );
        }
        _ => {}
    }
    ptr::null_mut()
}
/// Notice op0=initialized(disabled),1=hello active,2=invalid JSON. Category is a
/// controlled-host factory returning an owned ITfCategoryMgr; null uses COM.
/// # Safety
/// Live state/lifetime/manager on their TSF thread. Sinks points to four borrowed
/// interfaces (thread,key,compartment,edit). Lifetime pins state/context/code
/// during activation. Select/read/toggle/context remain live until owned UI is
/// detached/destroyed; other callbacks and arguments are synchronous. Worker is
/// a trusted absolute UTF-8 path. No callback unwinds or retains stack arguments.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_activate(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    manager: *mut c_void,
    client: u32,
    flags: u32,
    sinks: *const *mut c_void,
    context: *mut c_void,
    select: Option<Select>,
    read: Option<Read>,
    toggle: Option<Toggle>,
    notice: Option<Notice>,
    factory: Option<Category>,
) -> HRESULT {
    let (Some(select), Some(read), Some(toggle)) = (select, read, toggle) else {
        return E_INVALIDARG;
    };
    if state.is_null()
        || lifetime.is_null()
        || worker.is_null()
        || manager.is_null()
        || sinks.is_null()
    {
        return E_INVALIDARG;
    }
    let sinks = std::slice::from_raw_parts(sinks, 4);
    if sinks.iter().any(|p| p.is_null()) {
        return E_INVALIDARG;
    }
    let mut owner = Owner {
        state,
        lifetime,
        worker,
        edit_sink: sinks[3],
        context,
        select,
        read,
        toggle,
        notice,
        category: factory,
    };
    let callbacks = startup::Callbacks {
        owner: ptr::addr_of_mut!(owner).cast(),
        lifetime,
        epoch: ptr::addr_of_mut!((*state).focus_epoch),
        phase: ptr::addr_of_mut!((*state).shutdown_phase),
        manager: ptr::addr_of_mut!((*state).manager),
        keys: ptr::addr_of_mut!((*state).keys),
        abandoned: ptr::addr_of_mut!((*state).abandoned),
        resume: ptr::addr_of_mut!((*state).resume),
        subscriptions: [
            ptr::addr_of_mut!((*state).thread_subscription),
            ptr::addr_of_mut!((*state).key_subscription),
            ptr::addr_of_mut!((*state).mode_subscription),
        ],
        atoms: [
            ptr::addr_of_mut!((*state).atom_input),
            ptr::addr_of_mut!((*state).atom_converted),
            ptr::addr_of_mut!((*state).atom_target),
        ],
        sinks: [sinks[0], sinks[1], sinks[2]],
        initialize,
        action,
        category: factory.map(|_| category as unsafe extern "C" fn(*mut c_void) -> *mut c_void),
    };
    startup::meltype_tsf_startup(manager, client, flags, &callbacks)
}
