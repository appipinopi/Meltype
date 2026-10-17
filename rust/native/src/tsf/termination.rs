// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Host-termination notification policy. Captures recoverable preedit without
//! inserting returned commits, and never detaches a replacement composition.
use super::{abandoned, recovery};
use serde_json::{json, Value};
use std::{
    ffi::{c_char, c_void, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};
use windows::{
    core::{IUnknown, Interface, HRESULT},
    Win32::{Foundation::*, UI::TextServices::*},
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Callbacks {
    pub(super) owner: *mut c_void,
    pub(super) lifetime: *mut c_void,
    pub(super) composition: *mut *mut c_void,
    pub(super) context: *mut *mut c_void,
    pub(super) resume: *mut *mut c_void,
    pub(super) abandoned: *mut c_void,
    pub(super) read: unsafe extern "C" fn(*mut c_void) -> u32,
    pub(super) update: unsafe extern "C" fn(*mut c_void, u32),
    pub(super) request: unsafe extern "C" fn(*mut c_void, *const c_char),
}
unsafe fn same(a: &ITfComposition, b: &ITfComposition) -> bool {
    a.as_raw() == b.as_raw()
        || a.cast::<IUnknown>()
            .ok()
            .zip(b.cast::<IUnknown>().ok())
            .is_some_and(|(a, b)| a == b)
}
/// # Safety
/// TSF thread, valid cookie and borrowed composition. Distinct slots contain
/// owned composition/context COM refs and a thread-affine recovery token. They
/// remain writable while lifetime is retained. Callbacks never throw; metadata
/// is UTF-8 {sid:string}. read bits disabled=1,deactivated=2,converting=4.
/// update: 0 hide candidates,1 reset stale/clear converting,2 clear direct word
/// and mark moved. request sends commit(moved=true) but must not apply its reply.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_composition_terminated(
    cookie: u32,
    composition: *mut c_void,
    metadata: *const c_char,
    callbacks: *const Callbacks,
) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(c) = callbacks.as_ref().copied() else {
            return E_INVALIDARG;
        };
        let Some(_owner) = IUnknown::from_raw_borrowed(&c.lifetime).cloned() else {
            return E_INVALIDARG;
        };
        if c.composition.is_null()
            || c.context.is_null()
            || c.resume.is_null()
            || c.composition == c.context
            || c.composition == c.resume
            || c.context == c.resume
            || metadata.is_null()
        {
            return E_INVALIDARG;
        }
        let Ok(metadata) = serde_json::from_slice::<Value>(CStr::from_ptr(metadata).to_bytes())
        else {
            return E_INVALIDARG;
        };
        if !metadata["sid"].is_string() {
            return E_INVALIDARG;
        }
        let Some(target) = ITfComposition::from_raw_borrowed(&composition).cloned() else {
            return S_OK;
        };
        abandoned::meltype_tsf_clear_composition_attribute(target.as_raw(), cookie);
        let original = c.composition.read();
        let current = ITfComposition::from_raw_borrowed(&original).cloned();
        if current
            .as_ref()
            .is_none_or(|current| !same(&target, current))
        {
            abandoned::meltype_tsf_abandoned_forget(c.abandoned, target.as_raw());
            return S_OK;
        }
        if c.composition.read() != original {
            return S_OK;
        }
        let original_context = c.context.read();
        let context = ITfContext::from_raw_borrowed(&original_context).cloned();
        let old = c.resume.replace(ptr::null_mut());
        if !old.is_null() {
            recovery::meltype_tsf_destroy_resume(old);
        }
        if c.composition.read() != original || c.context.read() != original_context {
            return S_OK;
        }
        let flags = (c.read)(c.owner);
        if flags & 7 == 0 && context.is_some() {
            let captured =
                recovery::meltype_tsf_capture_resume(original_context, target.as_raw(), cookie);
            if c.composition.read() != original
                || c.context.read() != original_context
                || !c.resume.read().is_null()
            {
                if !captured.is_null() {
                    recovery::meltype_tsf_destroy_resume(captured);
                }
                return S_OK;
            }
            c.resume.write(captured);
        }
        // Detach both slots before releases, which can synchronously notify.
        if c.composition.read() != original || c.context.read() != original_context {
            return S_OK;
        }
        let old = c.composition.replace(ptr::null_mut());
        let old_context = c.context.replace(ptr::null_mut());
        if !old.is_null() {
            drop(ITfComposition::from_raw(old));
        }
        if !old_context.is_null() {
            drop(ITfContext::from_raw(old_context));
        }
        if !c.composition.read().is_null() {
            return S_OK;
        }
        (c.update)(c.owner, 0);
        if !c.composition.read().is_null() {
            return S_OK;
        }
        (c.update)(c.owner, 1);
        if !c.composition.read().is_null() || !c.resume.read().is_null() {
            return S_OK;
        }
        (c.update)(c.owner, 2);
        if (c.read)(c.owner) & 1 == 0 {
            let Ok(request) =
                CString::new(json!({"op":"commit","sid":metadata["sid"],"moved":true}).to_string())
            else {
                return E_FAIL;
            };
            (c.request)(c.owner, request.as_ptr());
        }
        S_OK
    }))
    .unwrap_or(E_FAIL)
}
