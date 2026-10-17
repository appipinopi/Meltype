// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Focus transitions: commit before switching edit notifications and metadata.
use super::key_state;
use std::{
    ffi::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
};
use windows::{
    core::{IUnknown, Interface, HRESULT},
    Win32::{Foundation::*, UI::TextServices::ITfDocumentMgr},
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Callbacks {
    owner: *mut c_void,
    lifetime: *mut c_void,
    epoch: *mut u64,
    read: unsafe extern "C" fn(*mut c_void) -> u32,
    commit: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    action: unsafe extern "C" fn(*mut c_void, u32, *mut c_void),
}
unsafe fn current(c: &Callbacks, epoch: u64) -> bool {
    if c.epoch.read() != epoch {
        return false;
    }
    let flags = (c.read)(c.owner);
    c.epoch.read() == epoch && flags & 4 == 0
}
/// # Safety
/// TSF thread, borrowed document/owner, writable epoch valid while lifetime is
/// retained. Callbacks never throw. read bits composition=1,disabled=2,ended=4.
/// commit mode 0=sync keep caret (nonzero success),1=async keep caret.
/// action 0=resume/commit null context,1=clear direct word/mark moved,2=clear field
/// cache,3=hide candidates,4=advise document,5=hello. Nested focus wins.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_document_focus(
    document: *mut c_void,
    callbacks: *const Callbacks,
) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(c) = callbacks.as_ref().copied() else {
            return E_INVALIDARG;
        };
        if c.epoch.is_null() {
            return E_INVALIDARG;
        }
        let epoch = c.epoch.read().wrapping_add(1);
        c.epoch.write(epoch);
        let Some(_owner) = IUnknown::from_raw_borrowed(&c.lifetime).cloned() else {
            return E_INVALIDARG;
        };
        if !current(&c, epoch) {
            return S_OK;
        }
        let document = ITfDocumentMgr::from_raw_borrowed(&document).cloned();
        if !current(&c, epoch) {
            return S_OK;
        }
        let flags = (c.read)(c.owner);
        if !current(&c, epoch) {
            return S_OK;
        }
        if flags & 1 != 0 {
            let committed = (c.commit)(c.owner, 0);
            if !current(&c, epoch) {
                return S_OK;
            }
            if committed == 0 {
                (c.commit)(c.owner, 1);
                if !current(&c, epoch) {
                    return S_OK;
                }
            }
        }
        let raw = document
            .as_ref()
            .map_or(std::ptr::null_mut(), Interface::as_raw);
        for op in 0..5 {
            (c.action)(c.owner, op, raw);
            if !current(&c, epoch) {
                return S_OK;
            }
        }
        let flags = (c.read)(c.owner);
        if current(&c, epoch) && flags & 2 == 0 {
            (c.action)(c.owner, 5, raw);
        }
        S_OK
    }))
    .unwrap_or(E_FAIL)
}
/// # Safety
/// Same callback/owner lifetime contract as document_focus. key_state is a Rust
/// key-state token on this TSF thread. Losing foreground forgets consumed keys
/// after hiding candidates; gaining foreground has no side effects.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_foreground_focus(
    foreground: i32,
    key_state: *mut c_void,
    callbacks: *const Callbacks,
) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(c) = callbacks.as_ref().copied() else {
            return E_INVALIDARG;
        };
        let Some(_owner) = IUnknown::from_raw_borrowed(&c.lifetime).cloned() else {
            return E_INVALIDARG;
        };
        if foreground == 0 {
            (c.action)(c.owner, 3, std::ptr::null_mut());
            key_state::meltype_tsf_key_foreground(key_state, 0);
        }
        S_OK
    }))
    .unwrap_or(E_FAIL)
}
