// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Recover host-committed preedit in a synchronous edit, or notify a moved
//! commit without reinserting text. No borrowed shell stack survives return.
use super::{composition, edit_session, recovery};
use serde_json::{json, Value};
use std::{
    cell::Cell,
    ffi::{c_char, c_void, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    rc::Rc,
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
    pub(super) sink: *mut c_void,
    pub(super) edit_context: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub(super) update: unsafe extern "C" fn(*mut c_void, u32, i32),
    pub(super) request: unsafe extern "C" fn(*mut c_void, *const c_char),
}
struct Pending {
    token: Cell<*mut c_void>,
    active: Cell<bool>,
    resumed: Cell<bool>,
}
impl Pending {
    unsafe fn clear(&self) {
        let token = self.token.replace(ptr::null_mut());
        if !token.is_null() {
            recovery::meltype_tsf_destroy_resume(token);
        }
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        unsafe {
            self.clear();
        }
    }
}
struct Work {
    c: Callbacks,
    _owner: IUnknown,
    context: ITfContext,
    pending: Rc<Pending>,
    atom: u32,
}
unsafe extern "C" fn run(owner: *mut c_void, cookie: u32) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let w = &*owner.cast::<Work>();
        let c = w.c;
        if !w.pending.active.get() {
            return E_ABORT;
        }
        if !c.composition.read().is_null() {
            return S_OK;
        }
        if (c.edit_context)(c.owner) == w.context.as_raw() {
            (c.update)(c.owner, 0, 1);
        }
        let token = w.pending.token.replace(ptr::null_mut());
        let raw = recovery::meltype_tsf_try_resume(token, w.context.as_raw(), c.sink, cookie);
        if raw.is_null() {
            return S_OK;
        }
        let target = ITfComposition::from_raw(raw);
        if !c.composition.read().is_null() {
            if c.composition.read() != target.as_raw() {
                composition::meltype_tsf_end_composition(
                    w.context.as_raw(),
                    target.as_raw(),
                    cookie,
                );
            }
            return S_OK;
        }
        let old_context = c.context.replace(w.context.clone().into_raw());
        c.composition.write(target.clone().into_raw());
        if !old_context.is_null() {
            drop(ITfContext::from_raw(old_context));
        }
        if c.composition.read() != target.as_raw() || c.context.read() != w.context.as_raw() {
            return S_OK;
        }
        w.pending.resumed.set(true);
        // Publish the owned pair before styling, which can notify termination.
        recovery::meltype_tsf_resume_attribute(w.context.as_raw(), target.as_raw(), cookie, w.atom);
        S_OK
    }))
    .unwrap_or(E_FAIL)
}
unsafe extern "C" fn destroy(owner: *mut c_void) {
    drop(Box::from_raw(owner.cast::<Work>()));
}
/// # Safety
/// TSF thread, distinct writable slots containing owned COM refs/recovery token
/// and borrowed context/sink. Slots/callback state survive while lifetime is held;
/// callbacks never throw. metadata={sid:string}. update op0 own edit(value),op1
/// clear direct word/mark moved. request sends commit(moved=true), without applying
/// its reply. A delayed SYNC session is disabled and its token released on return.
/// Returns 0=no token,1=resumed,2=fallback commit requested,-1=invalid; replacement
/// input created while processing is preserved and returns 0 without fallback.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_resume_or_commit(
    context: *mut c_void,
    client: u32,
    atom: u32,
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
            || c.resume.is_null()
            || c.composition == c.context
            || c.context == c.resume
            || c.composition == c.resume
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
        let token = c.resume.replace(ptr::null_mut());
        if token.is_null() {
            return 0;
        }
        let original = c.composition.read();
        let _original = ITfComposition::from_raw_borrowed(&original).cloned();
        let pending = Rc::new(Pending {
            token: Cell::new(token),
            active: Cell::new(true),
            resumed: Cell::new(false),
        });
        let context = ITfContext::from_raw_borrowed(&context).cloned();
        if original.is_null()
            && context
                .as_ref()
                .is_some_and(|ctx| recovery::meltype_tsf_resume_matches(token, ctx.as_raw()) != 0)
        {
            let context = context.as_ref().unwrap();
            let work = Box::new(Work {
                c,
                _owner: owner.clone(),
                context: context.clone(),
                pending: pending.clone(),
                atom,
            });
            let _ = edit_session::meltype_tsf_request_edit_session(
                context.as_raw(),
                client,
                (TF_ES_SYNC | TF_ES_READWRITE).0,
                Box::into_raw(work).cast(),
                Some(run),
                destroy,
            );
            pending.active.set(false);
            (c.update)(c.owner, 0, 0);
        }
        pending.active.set(false);
        pending.clear();
        if pending.resumed.get() {
            return 1;
        }
        if c.composition.read() != original || !c.resume.read().is_null() {
            return 0;
        }
        (c.update)(c.owner, 1, 0);
        let Ok(request) =
            CString::new(json!({"op":"commit","sid":metadata["sid"],"moved":true}).to_string())
        else {
            return -1;
        };
        (c.request)(c.owner, request.as_ptr());
        2
    }))
    .unwrap_or(-1)
}
