// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Edit notifications using service-owned composition and caret state.
use super::*;
use crate::tsf::edit_events;
type Query = unsafe extern "C" fn(*mut c_void, u32, *mut c_void, i32);
type Commit = unsafe extern "C" fn(*mut c_void);
struct Owner {
    state: *mut ServiceState,
    context: *mut c_void,
    query: Query,
    commit: Commit,
}
unsafe extern "C" fn take_own(data: *mut c_void) -> i32 {
    let state = (*data.cast::<Owner>()).state;
    i32::from(ptr::addr_of_mut!((*state).own_edit).replace(false))
}
unsafe extern "C" fn current(data: *mut c_void, context: *mut *mut c_void) -> *mut c_void {
    let state = (*data.cast::<Owner>()).state;
    context.write((*state).composition_context);
    (*state).composition
}
unsafe extern "C" fn query(data: *mut c_void, cookie: u32, context: *mut c_void, changed: i32) {
    let owner = &*data.cast::<Owner>();
    (owner.query)(owner.context, cookie, context, changed);
}
unsafe extern "C" fn moved(data: *mut c_void, reset: i32) {
    let state = (*data.cast::<Owner>()).state;
    if reset != 0 {
        (*state).direct_word = false;
    }
    (*state).caret_moved = true;
}
unsafe extern "C" fn commit(data: *mut c_void) {
    let owner = &*data.cast::<Owner>();
    (owner.commit)(owner.context);
}
/// # Safety
/// Live service and TSF context/record on their owning thread with valid cookie.
/// Record may be null. Lifetime is an IUnknown pinning state/adapter/code through
/// callbacks. Query and commit callbacks must not unwind or retain arguments.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_edited(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    context: *mut c_void,
    record: *mut c_void,
    cookie: u32,
    adapter: *mut c_void,
    query_callback: Option<Query>,
    commit_callback: Option<Commit>,
) -> i32 {
    let (Some(query_callback), Some(commit_callback)) = (query_callback, commit_callback) else {
        return 0;
    };
    if state.is_null() || lifetime.is_null() {
        return 0;
    }
    let mut owner = Owner {
        state,
        context: adapter,
        query: query_callback,
        commit: commit_callback,
    };
    let callbacks = edit_events::Callbacks {
        owner: ptr::addr_of_mut!(owner).cast(),
        lifetime,
        take_own: Some(take_own),
        current: Some(current),
        query: Some(query),
        moved: Some(moved),
        commit: Some(commit),
    };
    edit_events::meltype_tsf_edit_notification(context, record, cookie, &callbacks)
}
