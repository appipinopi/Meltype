// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Commit scheduling and caret-preservation policy for current compositions.
use super::edit_session;
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
    owner: *mut c_void,
    lifetime: *mut c_void,
    current: unsafe extern "C" fn(*mut c_void, *mut *mut c_void, *mut *mut c_void) -> *mut c_void,
    update: unsafe extern "C" fn(*mut c_void, u32, u32, i32),
    request_apply: unsafe extern "C" fn(*mut c_void, *const c_char, u32, *mut c_void, *mut c_void),
}
struct Work {
    c: Callbacks,
    _owner: IUnknown,
    context: ITfContext,
    target: ITfComposition,
    request: CString,
    keep: bool,
    skip_stale: bool,
    active: Rc<Cell<bool>>,
    executed: Rc<Cell<bool>>,
}
impl Work {
    unsafe fn matches(&self) -> (bool, bool) {
        let (mut context, mut sink) = (ptr::null_mut(), ptr::null_mut());
        let target = (self.c.current)(self.c.owner, &mut context, &mut sink);
        (
            target == self.target.as_raw() && context == self.context.as_raw(),
            sink == self.context.as_raw(),
        )
    }
    unsafe fn run(&self, cookie: u32) -> HRESULT {
        if !self.active.get() {
            return E_ABORT;
        }
        self.executed.set(true);
        let (matches, visible) = self.matches();
        if !matches && self.skip_stale {
            return S_OK;
        }
        if matches {
            if visible {
                (self.c.update)(self.c.owner, 0, cookie, 1);
            }
            (self.c.update)(self.c.owner, 1, cookie, i32::from(self.keep));
            (self.c.request_apply)(
                self.c.owner,
                self.request.as_ptr(),
                cookie,
                self.context.as_raw(),
                self.target.as_raw(),
            );
            (self.c.update)(self.c.owner, 1, cookie, 0);
            // An apply callback can end the old composition and start another.
            // Do not end that new input when completing this request.
            if self.matches().0 {
                (self.c.update)(self.c.owner, 2, cookie, 0);
            }
        }
        (self.c.update)(self.c.owner, 3, cookie, 0);
        S_OK
    }
}
unsafe extern "C" fn run(owner: *mut c_void, cookie: u32) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| (&*owner.cast::<Work>()).run(cookie))).unwrap_or(E_FAIL)
}
unsafe extern "C" fn destroy(owner: *mut c_void) {
    drop(Box::from_raw(owner.cast::<Work>()));
}
/// # Safety
/// On the TSF thread, borrowed live interfaces and callbacks that never throw.
/// mode=0 sync,1 async,2 existing cookie/context. metadata={sid:string}. current
/// returns borrowed composition/context/edit-sink context. update operations:
/// 0 own edit,1 preserve caret,2 end current composition,3 hide candidates,
/// 4 clear own edit after a sync call. request_apply keeps its reply local and
/// must recheck target/context before applying a reply after a reentrant request.
/// For async callbacks owner state must survive until session cancellation or
/// execution; lifetime IUnknown is retained. Sync callbacks may only run until
/// return; incorrectly deferred SYNC sessions are disabled before return.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_commit(
    mode: u32,
    context: *mut c_void,
    client: u32,
    cookie: u32,
    metadata: *const c_char,
    callbacks: *const Callbacks,
    keep: i32,
) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(c) = callbacks.as_ref().copied() else {
            return E_INVALIDARG;
        };
        let Some(owner) = IUnknown::from_raw_borrowed(&c.lifetime).cloned() else {
            return E_INVALIDARG;
        };
        if mode > 2 || metadata.is_null() {
            return E_INVALIDARG;
        }
        let Ok(metadata) = serde_json::from_slice::<Value>(CStr::from_ptr(metadata).to_bytes())
        else {
            return E_INVALIDARG;
        };
        if !metadata["sid"].is_string() {
            return E_INVALIDARG;
        }
        let (mut composition_context, mut sink) = (ptr::null_mut(), ptr::null_mut());
        let target = (c.current)(c.owner, &mut composition_context, &mut sink);
        let target = ITfComposition::from_raw_borrowed(&target).cloned();
        let context = if mode == 2 {
            context
        } else {
            composition_context
        };
        let context = ITfContext::from_raw_borrowed(&context).cloned();
        let (Some(target), Some(context)) = (target, context) else {
            if mode == 2 {
                (c.update)(c.owner, 3, cookie, 0);
            }
            return S_OK;
        };
        let mut request = json!({"op":"commit","sid":metadata["sid"]});
        if keep != 0 {
            request["moved"] = json!(true);
        }
        let Ok(request) = CString::new(request.to_string()) else {
            return E_INVALIDARG;
        };
        let active = Rc::new(Cell::new(true));
        let executed = Rc::new(Cell::new(false));
        let work = Box::new(Work {
            c,
            _owner: owner,
            context: context.clone(),
            target,
            request,
            keep: keep != 0,
            skip_stale: mode != 2,
            active: active.clone(),
            executed: executed.clone(),
        });
        if mode == 2 {
            return work.run(cookie);
        }
        let flags = if mode == 0 {
            TF_ES_SYNC | TF_ES_READWRITE
        } else {
            TF_ES_ASYNCDONTCARE | TF_ES_READWRITE
        };
        // Keep a separate owner reference while request scheduling can release
        // its Work and while sync cleanup still calls into the shell.
        let _owner = IUnknown::from_raw_borrowed(&c.lifetime).cloned();
        let status = edit_session::meltype_tsf_request_edit_session(
            context.as_raw(),
            client,
            flags.0,
            Box::into_raw(work).cast(),
            Some(run),
            destroy,
        );
        if mode == 0 {
            active.set(false);
            (c.update)(c.owner, 4, 0, 0);
            if status.is_ok() && !executed.get() {
                return E_ABORT;
            }
        }
        status
    }))
    .unwrap_or(E_FAIL)
}
