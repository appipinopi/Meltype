// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Composition operations and post-end key cleanup against service-owned slots.
use super::*;
use crate::tsf::composition_owner;
unsafe fn frame(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
) -> Option<(composition_owner::Slots, IUnknown)> {
    if state.is_null() {
        return None;
    }
    let pin = IUnknown::from_raw_borrowed(&lifetime)?.clone();
    Some((
        composition_owner::Slots {
            lifetime,
            composition: ptr::addr_of_mut!((*state).composition),
            context: ptr::addr_of_mut!((*state).composition_context),
            sink,
        },
        pin,
    ))
}
/// # Safety
/// Live service and borrowed TSF lifetime/context/composition sink on their TSF
/// thread, valid edit cookie, NUL-terminated UTF-8 view. Lifetime pins state/code
/// across reentrant COM. Slots follow service-owned COM reference conventions.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_apply_composition(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    context: *mut c_void,
    cookie: u32,
    view: *const c_char,
) -> i32 {
    let Some((slots, _pin)) = frame(state, lifetime, sink) else {
        return -1;
    };
    composition_owner::meltype_tsf_apply_owned_composition(
        context,
        cookie,
        view,
        (*state).atom_input,
        (*state).atom_converted,
        (*state).atom_target,
        &slots,
    )
}
/// # Safety
/// Same service/COM ownership contract. Range and UTF-16 text are borrowed;
/// text may be null only for zero length. State keep-caret controls writing.
/// The outer lifetime pin extends through post-end stale-key cleanup.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_write_commit(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    context: *mut c_void,
    range: *mut c_void,
    cookie: u32,
    text: *const u16,
    length: u32,
) -> i32 {
    let Some((slots, _pin)) = frame(state, lifetime, sink) else {
        return -1;
    };
    let result = composition_owner::meltype_tsf_write_owned_commit(
        context,
        range,
        cookie,
        text,
        length,
        i32::from((*state).keep_caret),
        &slots,
    );
    if result >= 0 && result & 2 != 0 {
        key_state::meltype_tsf_key_stale_clear((*state).keys);
    }
    result
}
/// # Safety
/// Same service/COM ownership contract, valid edit cookie. Retains the service
/// through composition ending and stale-key cleanup; preserves replacements.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_end_composition(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    cookie: u32,
) -> i32 {
    let Some((slots, _pin)) = frame(state, lifetime, ptr::null_mut()) else {
        return -1;
    };
    let result = composition_owner::meltype_tsf_end_owned_composition(cookie, &slots);
    if result > 0 {
        key_state::meltype_tsf_key_stale_clear((*state).keys);
    }
    result
}
