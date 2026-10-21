// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Retain the actual notification source used during subscription, so teardown
//! never queries a replacement source or releases COM under a token-map borrow.
use super::fields;
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};
use windows::{
    core::{IUnknown, Interface, GUID, VARIANT},
    Win32::UI::TextServices::*,
};
enum Subscription {
    Source {
        source: ITfSource,
        cookie: u32,
    },
    Keys {
        manager: ITfKeystrokeMgr,
        client: u32,
    },
}
impl Drop for Subscription {
    fn drop(&mut self) {
        unsafe {
            match self {
                Self::Source { source, cookie } => {
                    let _ = source.UnadviseSink(*cookie);
                }
                Self::Keys { manager, client } => {
                    let _ = manager.UnadviseKeyEventSink(*client);
                }
            }
        }
    }
}
thread_local! {static SUBSCRIPTIONS:RefCell<HashMap<usize,Subscription>>=RefCell::new(HashMap::new());}
static NEXT: AtomicUsize = AtomicUsize::new(1);
unsafe fn source(owner: &IUnknown, iid: &GUID, sink: &IUnknown) -> Option<Subscription> {
    let source = fields::query::<ITfSource>(owner)?;
    let mut cookie = u32::MAX;
    let status =
        (Interface::vtable(&source).AdviseSink)(source.as_raw(), iid, sink.as_raw(), &mut cookie);
    if status.is_err() {
        if cookie != u32::MAX {
            let _ = source.UnadviseSink(cookie);
        }
        return None;
    }
    if cookie == u32::MAX {
        return None;
    }
    Some(Subscription::Source { source, cookie })
}
unsafe fn top(owner: &IUnknown) -> Option<ITfContext> {
    // This input is an ITfDocumentMgr, not queried for a different interface.
    let pointer = owner.as_raw();
    let document = ITfDocumentMgr::from_raw_borrowed(&pointer)?;
    let mut raw = ptr::null_mut();
    let status = (Interface::vtable(document).GetTop)(document.as_raw(), &mut raw);
    let context = (!raw.is_null()).then(|| ITfContext::from_raw(raw));
    if status.is_ok() {
        context
    } else {
        None
    }
}
/// Subscribe on the TSF thread. Kind: 0=thread events, 1=foreground keys,
/// 2=open/close (set Japanese first), 3=top-context edit events. Edit subscription
/// writes an owned ITfContext to context_out on success, null on failure.
/// # Safety
/// Borrowed live owner/sink interfaces on their thread: owner is ITfThreadMgr
/// except kind 3 (ITfDocumentMgr). Sink implements the selected event interface.
/// context_out is writable and required for kind 3. Unsubscribe on this thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_subscribe(
    kind: u32,
    owner: *mut c_void,
    client: u32,
    sink: *mut c_void,
    context_out: *mut *mut c_void,
) -> *mut c_void {
    if !context_out.is_null() {
        context_out.write(ptr::null_mut());
    }
    catch_unwind(AssertUnwindSafe(|| {
        let owner = IUnknown::from_raw_borrowed(&owner)?.clone();
        let sink = IUnknown::from_raw_borrowed(&sink)?.clone();
        let mut context = None;
        let subscription = match kind {
            0 => source(&owner, &ITfThreadMgrEventSink::IID, &sink)?,
            1 => {
                let manager = fields::query::<ITfKeystrokeMgr>(&owner)?;
                let pointer = sink.as_raw();
                let key = ITfKeyEventSink::from_raw_borrowed(&pointer)?;
                manager.AdviseKeyEventSink(client, key, true).ok()?;
                Subscription::Keys { manager, client }
            }
            2 => {
                let manager = fields::manager(owner.as_raw())?;
                let compartment =
                    fields::compartment(&manager, &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)?;
                let _ = compartment.SetValue(client, &VARIANT::from(1i32));
                source(
                    &compartment.cast::<IUnknown>().ok()?,
                    &ITfCompartmentEventSink::IID,
                    &sink,
                )?
            }
            3 => {
                if context_out.is_null() {
                    return None;
                }
                let current = top(&owner)?;
                let subscription = source(
                    &current.cast::<IUnknown>().ok()?,
                    &ITfTextEditSink::IID,
                    &sink,
                )?;
                context = Some(current);
                subscription
            }
            _ => return None,
        };
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        SUBSCRIPTIONS.with(|m| m.borrow_mut().insert(id, subscription));
        if let Some(context) = context {
            context_out.write(context.into_raw());
        }
        Some(id as *mut c_void)
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}
/// Removing before Unadvise makes repeated/reentrant unsubscription harmless.
/// Stale/foreign-thread tokens cannot access another thread's COM sources.
#[no_mangle]
pub extern "C" fn meltype_tsf_unsubscribe(handle: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let value = SUBSCRIPTIONS
            .try_with(|m| {
                m.try_borrow_mut()
                    .ok()
                    .and_then(|mut m| m.remove(&(handle as usize)))
            })
            .ok()
            .flatten();
        drop(value);
    }));
}
