// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Activation ordering and ownership; UI/state adapters remain in the shell.
use super::{abandoned, fields, key_state, recovery, subscriptions, SPECS};
use std::{
    ffi::{c_char, c_void, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};
use windows::{
    core::{IUnknown, Interface, HRESULT},
    Win32::{
        Foundation::*,
        System::{
            Com::*,
            SystemInformation::GetTickCount64,
            Threading::{GetCurrentProcessId, GetCurrentThreadId},
        },
        UI::TextServices::*,
    },
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Callbacks {
    owner: *mut c_void,
    lifetime: *mut c_void,
    epoch: *mut u64,
    phase: *mut u32,
    manager: *mut *mut c_void,
    keys: *mut *mut c_void,
    abandoned: *mut *mut c_void,
    resume: *mut *mut c_void,
    subscriptions: [*mut *mut c_void; 3],
    atoms: [*mut u32; 3],
    sinks: [*mut c_void; 3],
    initialize: unsafe extern "C" fn(*mut c_void, u32, i32, i32, *const c_char),
    action: unsafe extern "C" fn(*mut c_void, u32, i32, *mut c_void) -> *mut c_void,
    category: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
}
unsafe fn current(c: &Callbacks, epoch: u64) -> bool {
    c.epoch.read() == epoch && c.phase.read() == 0
}
/// # Safety
/// TSF thread; borrowed live manager/owner/sinks; distinct writable owned COM /
/// Rust-token slots, atom slots, epoch/phase and callback storage survive while
/// lifetime is held. Callbacks never throw. Subscription/sink order thread,key,mode.
/// initialize resets shell primitive state and stores client/disabled/own/sid.
/// action 0 sets open(value),1 creates candidates,2 creates bar and returns borrowed
/// ITfLangBarItem,3 advises document,4 hello. category optionally returns owned
/// ITfCategoryMgr for controlled hosts; null callback uses CoCreateInstance.
/// Nested activation/shutdown supersedes remaining work; late tokens are undone.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_startup(
    manager: *mut c_void,
    client: u32,
    flags: u32,
    callbacks: *const Callbacks,
) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(c) = callbacks.as_ref().copied() else {
            return E_INVALIDARG;
        };
        let slots = [
            c.manager,
            c.keys,
            c.abandoned,
            c.resume,
            c.subscriptions[0],
            c.subscriptions[1],
            c.subscriptions[2],
        ];
        if c.epoch.is_null()
            || c.phase.is_null()
            || manager.is_null()
            || c.lifetime.is_null()
            || slots
                .iter()
                .enumerate()
                .any(|(i, p)| p.is_null() || slots[..i].contains(p))
            || c.atoms
                .iter()
                .enumerate()
                .any(|(i, p)| p.is_null() || c.atoms[..i].contains(p))
            || c.sinks.iter().any(|p| p.is_null())
        {
            return E_INVALIDARG;
        }
        let epoch = c.epoch.read().wrapping_add(1);
        c.epoch.write(epoch);
        c.phase.write(0);
        let _owner = IUnknown::from_raw_borrowed(&c.lifetime).cloned().unwrap();
        if !current(&c, epoch) {
            return S_OK;
        }
        let manager = ITfThreadMgr::from_raw_borrowed(&manager).cloned().unwrap();
        if !current(&c, epoch) {
            return S_OK;
        }
        let owned_manager = manager.clone();
        if !current(&c, epoch) {
            return S_OK;
        }
        let old = c.manager.replace(owned_manager.into_raw());
        if !old.is_null() {
            drop(IUnknown::from_raw(old));
        }
        if !current(&c, epoch) {
            return S_OK;
        }
        for slot in c.subscriptions {
            subscriptions::meltype_tsf_unsubscribe(slot.replace(ptr::null_mut()));
            if !current(&c, epoch) {
                return S_OK;
            }
        }
        let disabled = flags & TF_TMAE_SECUREMODE != 0;
        let sid = CString::new(format!(
            "{}-{}-{}",
            GetCurrentProcessId(),
            GetCurrentThreadId(),
            GetTickCount64()
        ))
        .unwrap();
        (c.initialize)(
            c.owner,
            client,
            i32::from(disabled),
            i32::from(super::process::own_process()),
            sid.as_ptr(),
        );
        if !current(&c, epoch) {
            return S_OK;
        }
        if c.keys.read().is_null() {
            c.keys.write(key_state::meltype_tsf_key_state_create());
        }
        key_state::meltype_tsf_key_state_reset(c.keys.read());
        if !current(&c, epoch) {
            return S_OK;
        }
        recovery::meltype_tsf_destroy_resume(c.resume.replace(ptr::null_mut()));
        if !current(&c, epoch) {
            return S_OK;
        }
        if c.abandoned.read().is_null() {
            c.abandoned.write(abandoned::meltype_tsf_abandoned_create());
        }
        abandoned::meltype_tsf_abandoned_reset(c.abandoned.read());
        if !current(&c, epoch) {
            return S_OK;
        }
        for kind in 0..2 {
            let token = subscriptions::meltype_tsf_subscribe(
                kind,
                manager.as_raw(),
                client,
                c.sinks[kind as usize],
                ptr::null_mut(),
            );
            if !current(&c, epoch) {
                subscriptions::meltype_tsf_unsubscribe(token);
                return S_OK;
            }
            c.subscriptions[kind as usize].write(token);
        }
        let category = match c.category {
            Some(create) => {
                let raw = create(c.owner);
                (!raw.is_null()).then(|| ITfCategoryMgr::from_raw(raw))
            }
            None => CoCreateInstance::<_, ITfCategoryMgr>(
                &CLSID_TF_CategoryMgr,
                None,
                CLSCTX_INPROC_SERVER,
            )
            .ok(),
        };
        if !current(&c, epoch) {
            return S_OK;
        }
        if let Some(category) = category {
            for (index, spec) in SPECS.iter().enumerate() {
                let value = category.RegisterGUID(&spec.guid);
                if !current(&c, epoch) {
                    return S_OK;
                }
                if let Ok(value) = value {
                    c.atoms[index].write(value);
                }
            }
        }
        if !current(&c, epoch) {
            return S_OK;
        }
        if !disabled {
            let token = subscriptions::meltype_tsf_subscribe(
                2,
                manager.as_raw(),
                client,
                c.sinks[2],
                ptr::null_mut(),
            );
            if !current(&c, epoch) {
                subscriptions::meltype_tsf_unsubscribe(token);
                return S_OK;
            }
            c.subscriptions[2].write(token);
        }
        (c.action)(c.owner, 0, i32::from(!disabled), ptr::null_mut());
        if !current(&c, epoch) {
            return S_OK;
        }
        (c.action)(c.owner, 1, 0, ptr::null_mut());
        if !current(&c, epoch) {
            return S_OK;
        }
        if !disabled {
            let bar = (c.action)(c.owner, 2, 0, ptr::null_mut());
            if !current(&c, epoch) {
                return S_OK;
            }
            let bar = ITfLangBarItem::from_raw_borrowed(&bar).cloned();
            if !current(&c, epoch) {
                return S_OK;
            }
            if let Some(bar) = bar {
                let raw = manager.as_raw();
                let unknown = IUnknown::from_raw_borrowed(&raw).unwrap();
                if let Some(items) = fields::query::<ITfLangBarItemMgr>(unknown) {
                    if !current(&c, epoch) {
                        return S_OK;
                    }
                    let _ = items.AddItem(&bar);
                }
            }
        }
        if !current(&c, epoch) {
            return S_OK;
        }
        let mut raw = ptr::null_mut();
        let result = (Interface::vtable(&manager).GetFocus)(manager.as_raw(), &mut raw);
        let focus = (!raw.is_null()).then(|| ITfDocumentMgr::from_raw(raw));
        if !current(&c, epoch) {
            return S_OK;
        }
        if result.is_ok() {
            if let Some(focus) = focus {
                (c.action)(c.owner, 3, 0, focus.as_raw());
            }
        }
        if !current(&c, epoch) {
            return S_OK;
        }
        if !disabled {
            (c.action)(c.owner, 4, 0, ptr::null_mut());
        }
        S_OK
    }))
    .unwrap_or(E_FAIL)
}
