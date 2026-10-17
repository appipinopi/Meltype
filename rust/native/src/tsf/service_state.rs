// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Rust-owned TSF state storage. Temporary C++ aliases use a checked C layout.
//! Raw interface/token slots retain their existing TSF-thread ownership policy.
use super::{fields, langbar};
use std::{
    alloc::{alloc, Layout},
    ffi::{c_char, c_void, CStr, CString},
    mem::{align_of, offset_of, size_of},
    panic::catch_unwind,
    ptr,
    sync::atomic::{AtomicU32, Ordering},
};
use windows::{
    core::{IUnknown, Interface},
    Win32::UI::TextServices::{ITfLangBarItem, ITfLangBarItemMgr},
};
#[repr(C)]
pub struct ServiceState {
    refs: i32,
    manager: *mut c_void,
    client: u32,
    thread_subscription: *mut c_void,
    key_subscription: *mut c_void,
    mode_subscription: *mut c_void,
    edit_subscription: *mut c_void,
    edit_context: *mut c_void,
    composition: *mut c_void,
    composition_context: *mut c_void,
    atom_input: u32,
    atom_converted: u32,
    atom_target: u32,
    keys: *mut c_void,
    resume: *mut c_void,
    abandoned: *mut c_void,
    last_hello: u64,
    focus_epoch: u64,
    mode_epoch: u64,
    shutdown_phase: u32,
    server_active: bool,
    direct_detection: bool,
    direct_word: bool,
    open: bool,
    code_app: bool,
    code_english: bool,
    disabled: bool,
    own_process: bool,
    last_converting: bool,
    caret_moved: bool,
    own_edit: bool,
    secret_checked: bool,
    secret_composing: bool,
    private_field: bool,
    keep_caret: bool,
    deactivated: bool,
    key_failed: bool,
    consumed: bool,
    scope_source: *const u16,
    pass_reason: *const u16,
    sid: *mut c_char,
    transport: *mut c_void,
    langbar: *mut c_void,
}
impl Default for ServiceState {
    fn default() -> Self {
        Self {
            refs: 1,
            manager: ptr::null_mut(),
            client: 0,
            thread_subscription: ptr::null_mut(),
            key_subscription: ptr::null_mut(),
            mode_subscription: ptr::null_mut(),
            edit_subscription: ptr::null_mut(),
            edit_context: ptr::null_mut(),
            composition: ptr::null_mut(),
            composition_context: ptr::null_mut(),
            atom_input: 0,
            atom_converted: 0,
            atom_target: 0,
            keys: ptr::null_mut(),
            resume: ptr::null_mut(),
            abandoned: ptr::null_mut(),
            last_hello: 0,
            focus_epoch: 0,
            mode_epoch: 0,
            shutdown_phase: 0,
            server_active: false,
            direct_detection: false,
            direct_word: false,
            open: true,
            code_app: false,
            code_english: false,
            disabled: false,
            own_process: false,
            last_converting: false,
            caret_moved: false,
            own_edit: false,
            secret_checked: false,
            secret_composing: false,
            private_field: false,
            keep_caret: false,
            deactivated: false,
            key_failed: false,
            consumed: true,
            scope_source: ptr::null(),
            pass_reason: ptr::null(),
            sid: ptr::null_mut(),
            transport: ptr::null_mut(),
            langbar: ptr::null_mut(),
        }
    }
}
impl Drop for ServiceState {
    fn drop(&mut self) {
        unsafe {
            self.shutdown_phase = 2;
            self.deactivated = true;
            retire_bar(ptr::addr_of_mut!(self.langbar).replace(ptr::null_mut()));
            let sid = std::mem::replace(&mut self.sid, ptr::null_mut());
            if !sid.is_null() {
                drop(CString::from_raw(sid));
            }
            close_transport(self);
        }
    }
}
unsafe fn retire_bar(raw: *mut c_void) {
    if !raw.is_null() {
        langbar::meltype_tsf_langbar_detach(raw);
        drop(IUnknown::from_raw(raw));
    }
}
/// Replace the owned bar and return a borrowed ITfLangBarItem. Publish the new
/// slot before detaching/releasing the old bar, whose sink release may reenter.
/// # Safety
/// Live state and callback context on their TSF thread for this entire call and
/// until removal/destruction. Caller retains the service across reentrant sink
/// releases. Callback signatures follow create_langbar and never unwind.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_create_langbar(
    state: *mut ServiceState,
    owner: *mut c_void,
    read: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    toggle: Option<unsafe extern "C" fn(*mut c_void)>,
) -> *mut c_void {
    if state.is_null() || (*state).shutdown_phase != 0 {
        return ptr::null_mut();
    }
    let epoch = (*state).focus_epoch;
    let created = langbar::meltype_tsf_create_langbar(owner, read, toggle);
    let previous = ptr::addr_of_mut!((*state).langbar).replace(created);
    retire_bar(previous);
    if (*state).shutdown_phase == 0 && (*state).focus_epoch == epoch && (*state).langbar == created
    {
        created
    } else {
        ptr::null_mut()
    }
}
/// # Safety
/// State is null or live on its TSF thread at entry. A local COM reference keeps
/// the bar alive if its notification destroys the service. No state access follows.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_update_langbar(state: *mut ServiceState) {
    if state.is_null() {
        return;
    }
    let raw = ptr::addr_of!((*state).langbar).read();
    if let Some(bar) = IUnknown::from_raw_borrowed(&raw).cloned() {
        langbar::meltype_tsf_langbar_update(bar.as_raw());
    }
}
/// Detach the callback context and slot before querying/removing from TSF.
/// # Safety
/// State is null or live on its TSF thread at entry. Manager is null or borrowed
/// IUnknown kept alive for the call. Reentrant replacement is left untouched.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_remove_langbar(
    state: *mut ServiceState,
    manager: *mut c_void,
) {
    if state.is_null() {
        return;
    }
    let raw = ptr::addr_of_mut!((*state).langbar).replace(ptr::null_mut());
    if raw.is_null() {
        return;
    }
    langbar::meltype_tsf_langbar_detach(raw);
    let bar = ITfLangBarItem::from_raw(raw);
    if let Some(manager) = IUnknown::from_raw_borrowed(&manager) {
        if let Some(items) = fields::query::<ITfLangBarItemMgr>(manager) {
            let _ = items.RemoveItem(&bar);
        }
    }
}
unsafe fn close_transport(state: *mut ServiceState) {
    let handle = ptr::addr_of_mut!((*state).transport).replace(ptr::null_mut());
    crate::tip_transport::meltype_tip_transport_destroy(handle);
}
/// # Safety
/// Live service state on its TSF thread; sid is a NUL-terminated byte string.
/// The bytes are copied before replacing old storage, so self-assignment is valid.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_set_sid(
    state: *mut ServiceState,
    sid: *const c_char,
) -> i32 {
    if state.is_null() || sid.is_null() {
        return 0;
    }
    let owned = CStr::from_ptr(sid).to_owned().into_raw();
    let old = ptr::addr_of_mut!((*state).sid).replace(owned);
    if !old.is_null() {
        drop(CString::from_raw(old));
    }
    1
}
/// Borrowed until the next set_sid or state destruction; null state returns null.
/// # Safety
/// state is null or live on its TSF thread; callers never free the returned bytes.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_sid(state: *const ServiceState) -> *const c_char {
    if state.is_null() {
        return ptr::null();
    }
    let sid = ptr::addr_of!((*state).sid).read();
    if sid.is_null() {
        c"".as_ptr()
    } else {
        sid
    }
}
/// # Safety
/// Live state on its TSF thread with no concurrent request. Worker and request are
/// borrowed NUL-terminated UTF-8, as for tip_transport_create/request. No COM or
/// shell callbacks occur. Returned text is owned and freed with meltype_free.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_request(
    state: *mut ServiceState,
    worker: *const c_char,
    request: *const c_char,
    timeout: u32,
    optional: i32,
) -> *mut c_char {
    if state.is_null() || worker.is_null() || request.is_null() {
        return ptr::null_mut();
    }
    let slot = ptr::addr_of_mut!((*state).transport);
    let mut handle = slot.read();
    if handle.is_null() {
        handle = crate::tip_transport::meltype_tip_transport_create(worker);
        if handle.is_null() {
            return ptr::null_mut();
        }
        slot.write(handle);
    }
    crate::tip_transport::meltype_tip_transport_request(handle, request, timeout, optional)
}
/// Detach the transport before stopping its worker. Repeated close is inert.
/// # Safety
/// state is null or live on its TSF thread; no concurrent request/destruction.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_close(state: *mut ServiceState) {
    if !state.is_null() {
        close_transport(state);
    }
}
static LIVE: AtomicU32 = AtomicU32::new(0);
/// Allocate a TSF-thread state with upstream defaults. Null means allocation
/// failure. The pointer is opaque except to a matching checked shell layout.
#[no_mangle]
pub extern "C" fn meltype_tsf_service_state_create() -> *mut ServiceState {
    catch_unwind(|| unsafe {
        let data = alloc(Layout::new::<ServiceState>()).cast::<ServiceState>();
        if data.is_null() {
            return data;
        }
        data.write(ServiceState::default());
        LIVE.fetch_add(1, Ordering::Relaxed);
        data
    })
    .unwrap_or(ptr::null_mut())
}
/// # Safety
/// data is null or a live allocation from service_state_create, freed once on
/// its TSF thread after all borrowed aliases/callbacks have ceased. Owned COM/
/// token slots must have been cleaned up by the service lifecycle beforehand.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_state_destroy(data: *mut ServiceState) {
    if !data.is_null() {
        drop(Box::from_raw(data));
        LIVE.fetch_sub(1, Ordering::Relaxed);
    }
}
/// # Safety
/// out, if nonnull with sufficient capacity, is writable for the returned count.
/// Metadata is version,size,alignment, then field offsets in the checked header.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_state_layout(out: *mut u32, capacity: u32) -> u32 {
    let values = [
        3,
        size_of::<ServiceState>() as u32,
        align_of::<ServiceState>() as u32,
        offset_of!(ServiceState, refs) as u32,
        offset_of!(ServiceState, manager) as u32,
        offset_of!(ServiceState, client) as u32,
        offset_of!(ServiceState, thread_subscription) as u32,
        offset_of!(ServiceState, key_subscription) as u32,
        offset_of!(ServiceState, mode_subscription) as u32,
        offset_of!(ServiceState, edit_subscription) as u32,
        offset_of!(ServiceState, edit_context) as u32,
        offset_of!(ServiceState, composition) as u32,
        offset_of!(ServiceState, composition_context) as u32,
        offset_of!(ServiceState, atom_input) as u32,
        offset_of!(ServiceState, atom_converted) as u32,
        offset_of!(ServiceState, atom_target) as u32,
        offset_of!(ServiceState, keys) as u32,
        offset_of!(ServiceState, resume) as u32,
        offset_of!(ServiceState, abandoned) as u32,
        offset_of!(ServiceState, last_hello) as u32,
        offset_of!(ServiceState, focus_epoch) as u32,
        offset_of!(ServiceState, mode_epoch) as u32,
        offset_of!(ServiceState, shutdown_phase) as u32,
        offset_of!(ServiceState, server_active) as u32,
        offset_of!(ServiceState, direct_detection) as u32,
        offset_of!(ServiceState, direct_word) as u32,
        offset_of!(ServiceState, open) as u32,
        offset_of!(ServiceState, code_app) as u32,
        offset_of!(ServiceState, code_english) as u32,
        offset_of!(ServiceState, disabled) as u32,
        offset_of!(ServiceState, own_process) as u32,
        offset_of!(ServiceState, last_converting) as u32,
        offset_of!(ServiceState, caret_moved) as u32,
        offset_of!(ServiceState, own_edit) as u32,
        offset_of!(ServiceState, secret_checked) as u32,
        offset_of!(ServiceState, secret_composing) as u32,
        offset_of!(ServiceState, private_field) as u32,
        offset_of!(ServiceState, keep_caret) as u32,
        offset_of!(ServiceState, deactivated) as u32,
        offset_of!(ServiceState, key_failed) as u32,
        offset_of!(ServiceState, consumed) as u32,
        offset_of!(ServiceState, scope_source) as u32,
        offset_of!(ServiceState, pass_reason) as u32,
        offset_of!(ServiceState, sid) as u32,
        offset_of!(ServiceState, transport) as u32,
        offset_of!(ServiceState, langbar) as u32,
    ];
    if !out.is_null() && capacity as usize >= values.len() {
        ptr::copy_nonoverlapping(values.as_ptr(), out, values.len());
    }
    values.len() as u32
}
#[no_mangle]
pub extern "C" fn meltype_tsf_service_state_live_count() -> u32 {
    LIVE.load(Ordering::Relaxed)
}
