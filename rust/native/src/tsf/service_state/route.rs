// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Public COM operation routing directly to native service implementations.
use super::*;
use crate::tsf::service_com::{meltype_tsf_service_com_interface, Call};
use windows::{
    core::GUID,
    Win32::{Foundation::E_NOTIMPL, UI::TextServices::*},
};
type Notice = unsafe extern "C" fn(*mut c_void, u32, i32);
unsafe fn get(root: *mut c_void, iid: &GUID) -> *mut c_void {
    meltype_tsf_service_com_interface(root, iid)
}
struct Owner {
    adapter: *mut c_void,
    notice: Option<Notice>,
}
unsafe extern "C" fn invalid(data: *mut c_void) {
    let o = &*data.cast::<Owner>();
    if let Some(notice) = o.notice {
        notice(o.adapter, 1, 0);
    }
}
/// # Safety
/// Live state/native service COM root and readable Call on their TSF thread.
/// Root pins state and notice code. Call operands follow service COM SDK ABI.
/// Optional notices follow mode/key/activation/shutdown codes respectively,
/// never unwind, and persist until UI teardown/pending commits finish. Unknown
/// operations return E_NOTIMPL. Key output must be writable (null is rejected).
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_route(
    state: *mut ServiceState,
    root: *mut c_void,
    worker: *const c_char,
    call: *const Call,
    adapter: *mut c_void,
    mode_notice: Option<Notice>,
    key_notice: Option<Notice>,
    activation_notice: Option<Notice>,
    shutdown_notice: Option<Notice>,
) -> HRESULT {
    if state.is_null() || root.is_null() || worker.is_null() || call.is_null() {
        return E_INVALIDARG;
    }
    let Some(_pin) = IUnknown::from_raw_borrowed(&root).cloned() else {
        return E_INVALIDARG;
    };
    let c = &*call;
    let sink = get(root, &ITfCompositionSink::IID);
    let edit = get(root, &ITfTextEditSink::IID);
    match c.op {
        0 | 1 => {
            let sinks = [
                get(root, &ITfThreadMgrEventSink::IID),
                get(root, &ITfKeyEventSink::IID),
                get(root, &ITfCompartmentEventSink::IID),
                edit,
                sink,
            ];
            super::activate::meltype_tsf_service_activate_native(
                state,
                root,
                worker,
                c.a,
                c.n1 as u32,
                if c.op == 1 { c.n2 as u32 } else { 0 },
                sinks.as_ptr(),
                adapter,
                activation_notice,
                mode_notice,
                None,
            )
        }
        2 => super::deactivate::meltype_tsf_service_shutdown(
            state,
            root,
            sink,
            worker,
            adapter,
            shutdown_notice,
        ),
        5 | 8 => super::notifications::meltype_tsf_service_focus_native(
            state,
            root,
            sink,
            edit,
            worker,
            if c.op == 5 { c.a } else { ptr::null_mut() },
            if c.op == 5 { -1 } else { i32::from(c.n1 != 0) },
            adapter,
            mode_notice,
        ),
        9..=12 => {
            if c.b.is_null() {
                return E_INVALIDARG;
            }
            let kind = match c.op {
                9 => 0,
                10 => 2,
                11 => 1,
                _ => 3,
            };
            let key = if kind >= 2 {
                if c.n1 < 256 {
                    c.n1 as u32
                } else {
                    256
                }
            } else {
                c.n1 as u32
            };
            let eaten = super::dispatch::meltype_tsf_service_dispatch_key(
                state,
                root,
                sink,
                worker,
                kind,
                if kind >= 2 { ptr::null_mut() } else { c.a },
                key,
                if kind >= 2 { 0 } else { c.n2 as usize as u64 },
                adapter,
                mode_notice,
                key_notice,
                None,
            );
            c.b.cast::<i32>().write(eaten);
            S_OK
        }
        14 => {
            let mut owner = Owner {
                adapter,
                notice: mode_notice,
            };
            super::recovered::meltype_tsf_service_terminated(
                state,
                root,
                worker,
                c.n1 as u32,
                c.a,
                ptr::addr_of_mut!(owner).cast(),
                Some(invalid),
            )
        }
        17 => super::notifications::meltype_tsf_service_opened_native(
            state,
            root,
            sink,
            worker,
            c.a.cast(),
            adapter,
            mode_notice,
        ),
        18 => {
            super::notifications::meltype_tsf_service_edited_native(
                state,
                root,
                sink,
                worker,
                c.a,
                c.b,
                c.n1 as u32,
                adapter,
                mode_notice,
            );
            S_OK
        }
        _ => E_NOTIMPL,
    }
}
