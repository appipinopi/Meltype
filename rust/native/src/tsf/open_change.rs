// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Open/close notifications: publish mode before committing and refreshing UI.
use super::fields;
use std::{
    ffi::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};
use windows::{
    core::{IUnknown, Interface, GUID, HRESULT},
    Win32::{Foundation::*, UI::TextServices::*},
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Callbacks {
    owner: *mut c_void,
    lifetime: *mut c_void,
    epoch: *mut u64,
    phase: *mut u32,
    current: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> u32,
    update: unsafe extern "C" fn(*mut c_void, u32, i32),
}
unsafe fn snapshot(c: &Callbacks, epoch: u64, manager: *mut c_void) -> Option<u32> {
    if c.epoch.read() != epoch || c.phase.read() != 0 {
        return None;
    }
    let mut current = ptr::null_mut();
    let flags = (c.current)(c.owner, &mut current);
    (c.epoch.read() == epoch && c.phase.read() == 0 && current == manager && flags & 4 == 0)
        .then_some(flags)
}
/// # Safety
/// TSF thread, borrowed GUID/owner; writable epoch/phase and callbacks/storage
/// survive while lifetime is retained. Callbacks never throw. current returns
/// borrowed thread manager and flags open=1,composing=2,ended=4. update 0 publishes
/// open(value) and clears direct word,1 async commit(keep caret=false),2 refresh
/// bar,3 log current mode. Nested mode changes supersede remaining old work.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_open_changed(
    guid: *const GUID,
    callbacks: *const Callbacks,
) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(guid) = guid.as_ref() else {
            return E_INVALIDARG;
        };
        if *guid != GUID_COMPARTMENT_KEYBOARD_OPENCLOSE {
            return S_OK;
        }
        let Some(c) = callbacks.as_ref().copied() else {
            return E_INVALIDARG;
        };
        if c.epoch.is_null() || c.phase.is_null() || c.lifetime.is_null() {
            return E_INVALIDARG;
        }
        if c.phase.read() != 0 {
            return S_OK;
        }
        let epoch = c.epoch.read().wrapping_add(1);
        c.epoch.write(epoch);
        let _owner = IUnknown::from_raw_borrowed(&c.lifetime).cloned().unwrap();
        if c.epoch.read() != epoch || c.phase.read() != 0 {
            return S_OK;
        }
        let mut manager = ptr::null_mut();
        let flags = (c.current)(c.owner, &mut manager);
        if flags & 4 != 0 || manager.is_null() || snapshot(&c, epoch, manager).is_none() {
            return S_OK;
        }
        let manager = ITfThreadMgr::from_raw_borrowed(&manager).cloned().unwrap();
        if snapshot(&c, epoch, manager.as_raw()).is_none() {
            return S_OK;
        }
        let value = fields::meltype_tsf_read_open(manager.as_raw());
        if value == -2 || snapshot(&c, epoch, manager.as_raw()).is_none() {
            return S_OK;
        }
        if value >= 0 {
            (c.update)(c.owner, 0, value);
            if snapshot(&c, epoch, manager.as_raw()).is_none() {
                return S_OK;
            }
        }
        (c.update)(c.owner, 3, 0);
        let Some(flags) = snapshot(&c, epoch, manager.as_raw()) else {
            return S_OK;
        };
        if flags & 1 == 0 && flags & 2 != 0 {
            (c.update)(c.owner, 1, 0);
            if snapshot(&c, epoch, manager.as_raw()).is_none() {
                return S_OK;
            }
        }
        (c.update)(c.owner, 2, 0);
        S_OK
    }))
    .unwrap_or(E_FAIL)
}
