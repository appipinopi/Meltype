// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Detach uneditable input before scheduling cleanup of the previous field.
use super::abandoned;
use serde_json::{json, Value};
use std::{
    ffi::{c_char, c_void, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};
use windows::core::{IUnknown, Interface};
use windows::Win32::UI::TextServices::{ITfComposition, ITfContext};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Callbacks {
    pub(super) owner: *mut c_void,
    pub(super) lifetime: *mut c_void,
    pub(super) composition: *mut *mut c_void,
    pub(super) context: *mut *mut c_void,
    pub(super) abandoned: *mut *mut c_void,
    pub(super) update: unsafe extern "C" fn(*mut c_void, u32),
    pub(super) request: unsafe extern "C" fn(*mut c_void, *const c_char),
}
unsafe fn replaced(c: &Callbacks) -> bool {
    !c.composition.read().is_null() || !c.context.read().is_null()
}
/// # Safety
/// TSF thread; distinct writable slots hold owned COM refs and an abandoned token.
/// Lifetime pins owner/storage/callback code. Callbacks never throw. update 0
/// resets stale state, 1 hides candidates, 2 clears direct word/marks caret moved.
/// request ignores the commit reply: document text must not be inserted again.
/// Returns 1=notified,0=reentrant replacement preserved,-1=invalid metadata/slots.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_abandon_composition(
    client: u32,
    metadata: *const c_char,
    callbacks: *const Callbacks,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(c) = callbacks.as_ref().copied() else {
            return -1;
        };
        let Some(owner) = IUnknown::from_raw_borrowed(&c.lifetime).cloned() else {
            return -1;
        };
        if c.composition.is_null()
            || c.context.is_null()
            || c.abandoned.is_null()
            || c.composition == c.context
            || c.composition == c.abandoned
            || c.context == c.abandoned
            || metadata.is_null()
        {
            return -1;
        }
        let Ok(metadata) = serde_json::from_slice::<Value>(CStr::from_ptr(metadata).to_bytes())
        else {
            return -1;
        };
        if !metadata["sid"].is_string() {
            return -1;
        }
        // Detach BOTH slots before AddRef/Release, RequestEditSession or UI callbacks.
        let raw = c.composition.replace(ptr::null_mut());
        let context = c.context.replace(ptr::null_mut());
        let old = (!raw.is_null()).then(|| ITfComposition::from_raw(raw));
        let context = (!context.is_null()).then(|| ITfContext::from_raw(context));
        if let (Some(old), Some(context)) = (&old, &context) {
            if c.abandoned.read().is_null() {
                c.abandoned.write(abandoned::meltype_tsf_abandoned_create());
            }
            let _ = abandoned::meltype_tsf_abandoned_add(
                c.abandoned.read(),
                context.as_raw(),
                old.as_raw(),
                owner.as_raw(),
                client,
            );
        }
        drop(old);
        drop(context);
        // A synchronous host or Release can have started another input. Never
        // reset its bookkeeping, hide its candidates or commit its core session.
        if replaced(&c) {
            return 0;
        }
        for op in 0..3 {
            (c.update)(c.owner, op);
            if replaced(&c) {
                return 0;
            }
        }
        let Ok(request) =
            CString::new(json!({"op":"commit","sid":metadata["sid"],"moved":true}).to_string())
        else {
            return -1;
        };
        (c.request)(c.owner, request.as_ptr());
        1
    }))
    .unwrap_or(-1)
}
