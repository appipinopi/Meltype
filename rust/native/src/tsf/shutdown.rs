// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Ordered, reentrant-safe TSF service shutdown. No borrowed stack survives sync.
use super::{edit_session, subscriptions};
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
    pub(super) epoch: *mut u64,
    pub(super) phase: *mut u32,
    pub(super) composition: *mut *mut c_void,
    pub(super) context: *mut *mut c_void,
    pub(super) manager: *mut *mut c_void,
    pub(super) mode_subscription: *mut *mut c_void,
    pub(super) key_subscription: *mut *mut c_void,
    pub(super) thread_subscription: *mut *mut c_void,
    pub(super) read: unsafe extern "C" fn(*mut c_void) -> u32,
    pub(super) action: unsafe extern "C" fn(*mut c_void, u32, u32, *mut c_void),
    pub(super) request:
        unsafe extern "C" fn(*mut c_void, *const c_char, u32, *mut c_void, *mut c_void),
}
unsafe fn current(c: &Callbacks, epoch: u64) -> bool {
    c.epoch.read() == epoch && c.phase.read() == 1
}
struct Work {
    c: Callbacks,
    epoch: u64,
    active: Rc<Cell<bool>>,
    _owner: IUnknown,
    context: ITfContext,
    composition: ITfComposition,
    request: CString,
}
unsafe extern "C" fn run(data: *mut c_void, cookie: u32) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let w = &*data.cast::<Work>();
        if !w.active.get() {
            return E_ABORT;
        }
        if !current(&w.c, w.epoch)
            || w.c.composition.read() != w.composition.as_raw()
            || w.c.context.read() != w.context.as_raw()
        {
            return S_OK;
        }
        (w.c.request)(
            w.c.owner,
            w.request.as_ptr(),
            cookie,
            w.context.as_raw(),
            w.composition.as_raw(),
        );
        if current(&w.c, w.epoch)
            && w.c.composition.read() == w.composition.as_raw()
            && w.c.context.read() == w.context.as_raw()
        {
            (w.c.action)(w.c.owner, 9, cookie, ptr::null_mut());
        }
        S_OK
    }))
    .unwrap_or(E_FAIL)
}
unsafe extern "C" fn destroy(data: *mut c_void) {
    drop(Box::from_raw(data.cast::<Work>()));
}
/// # Safety
/// TSF thread, live owner and distinct writable owned COM/subscription slots.
/// epoch/phase/storage/code survive while lifetime is held; callbacks never throw.
/// phase=0 active,1 stopping,2 stopped. Activation resets phase and advances epoch.
/// read bit0=disabled. action 0=mark deactivated,1=clear resume,2=clear field cache,
/// 3=end abandoned,4=close transport,5=drop candidates,6=unadvise edit,7=remove and
/// detach language bar(manager),8=reset client,9=end composition(cookie).
/// request applies commit reply only to matching context/target; ignores close.
/// Returns S_OK on repeated/nested shutdown or superseding activation.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_shutdown(
    client: u32,
    metadata: *const c_char,
    callbacks: *const Callbacks,
) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(c) = callbacks.as_ref().copied() else {
            return E_INVALIDARG;
        };
        let slots = [
            c.composition,
            c.context,
            c.manager,
            c.mode_subscription,
            c.key_subscription,
            c.thread_subscription,
        ];
        if c.epoch.is_null()
            || c.phase.is_null()
            || metadata.is_null()
            || slots
                .iter()
                .enumerate()
                .any(|(i, p)| p.is_null() || slots[..i].contains(p))
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
        let Some(owner) = IUnknown::from_raw_borrowed(&c.lifetime) else {
            return E_INVALIDARG;
        };
        if c.phase.read() != 0 {
            return S_OK;
        }
        c.phase.write(1);
        let epoch = c.epoch.read().wrapping_add(1);
        c.epoch.write(epoch);
        let owner = owner.clone();
        if !current(&c, epoch) {
            return S_OK;
        }
        let raw = c.context.read();
        let context = ITfContext::from_raw_borrowed(&raw).cloned();
        if !current(&c, epoch) {
            return S_OK;
        }
        let raw = c.composition.read();
        let composition = ITfComposition::from_raw_borrowed(&raw).cloned();
        if !current(&c, epoch) {
            return S_OK;
        }
        if let (Some(context), Some(composition)) = (context, composition) {
            let active = Rc::new(Cell::new(true));
            let request =
                CString::new(json!({"op":"commit","sid":metadata["sid"],"moved":true}).to_string())
                    .unwrap();
            let work = Box::new(Work {
                c,
                epoch,
                active: active.clone(),
                _owner: owner.clone(),
                context: context.clone(),
                composition,
                request,
            });
            let _ = edit_session::meltype_tsf_request_edit_session(
                context.as_raw(),
                client,
                (TF_ES_SYNC | TF_ES_READWRITE).0,
                Box::into_raw(work).cast(),
                Some(run),
                destroy,
            );
            active.set(false);
        }
        if !current(&c, epoch) {
            return S_OK;
        }
        let flags = (c.read)(c.owner);
        if !current(&c, epoch) {
            return S_OK;
        }
        if flags & 1 == 0 {
            let request =
                CString::new(json!({"op":"close","sid":metadata["sid"]}).to_string()).unwrap();
            (c.request)(
                c.owner,
                request.as_ptr(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
            if !current(&c, epoch) {
                return S_OK;
            }
        }
        for op in 0..7 {
            (c.action)(c.owner, op, 0, ptr::null_mut());
            if !current(&c, epoch) {
                return S_OK;
            }
        }
        // Keep the old manager alive while the language bar is removed. Remove
        // the bar's owner slot before COM can reenter (adapter contract).
        let raw = c.manager.read();
        let manager = IUnknown::from_raw_borrowed(&raw).cloned();
        if !current(&c, epoch) {
            return S_OK;
        }
        (c.action)(
            c.owner,
            7,
            0,
            manager.as_ref().map_or(ptr::null_mut(), Interface::as_raw),
        );
        drop(manager);
        if !current(&c, epoch) {
            return S_OK;
        }
        for slot in [
            c.mode_subscription,
            c.key_subscription,
            c.thread_subscription,
        ] {
            subscriptions::meltype_tsf_unsubscribe(slot.replace(ptr::null_mut()));
            if !current(&c, epoch) {
                return S_OK;
            }
        }
        let old = c.composition.replace(ptr::null_mut());
        let context = c.context.replace(ptr::null_mut());
        if !old.is_null() {
            drop(ITfComposition::from_raw(old));
        }
        if !context.is_null() {
            drop(ITfContext::from_raw(context));
        }
        if !current(&c, epoch) {
            return S_OK;
        }
        let manager = c.manager.replace(ptr::null_mut());
        if !manager.is_null() {
            drop(IUnknown::from_raw(manager));
        }
        if !current(&c, epoch) {
            return S_OK;
        }
        (c.action)(c.owner, 8, 0, ptr::null_mut());
        if current(&c, epoch) {
            c.phase.write(2);
        }
        S_OK
    }))
    .unwrap_or(E_FAIL)
}
