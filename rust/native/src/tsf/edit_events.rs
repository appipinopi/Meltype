// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Edit-notification control. The remaining owner exposes state/actions while
//! Rust reads TSF metadata, tests boundaries and rejects stale notifications.
use super::ranges;
use std::{
    ffi::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};
use windows::{
    core::{IUnknown, Interface},
    Win32::UI::{Input::KeyboardAndMouse::*, TextServices::*},
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Callbacks {
    pub(super) owner: *mut c_void,
    pub(super) lifetime: *mut c_void,
    pub(super) take_own: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub(super) current: Option<unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> *mut c_void>,
    pub(super) query: Option<unsafe extern "C" fn(*mut c_void, u32, *mut c_void, i32)>,
    pub(super) moved: Option<unsafe extern "C" fn(*mut c_void, i32)>,
    pub(super) commit: Option<unsafe extern "C" fn(*mut c_void)>,
}
/// Borrowed context/record and callbacks. Retains lifetime (IUnknown), context
/// and record across callbacks. commit requests preserving the application's
/// caret; query and commit never run for null records or consumed own edits.
/// # Safety
/// Live TSF interfaces and valid read cookie on their thread. Callback state and
/// code must survive until return; lifetime keeps their COM owner alive. Callbacks
/// must not throw across the ABI. current returns borrowed composition/context.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_edit_notification(
    context: *mut c_void,
    record: *mut c_void,
    cookie: u32,
    callbacks: *const Callbacks,
) -> i32 {
    i32::from(
        catch_unwind(AssertUnwindSafe(|| {
            if record.is_null() {
                return true;
            }
            let Some(callbacks) = callbacks.as_ref().copied() else {
                return false;
            };
            let (Some(take), Some(current), Some(query), Some(moved), Some(commit)) = (
                callbacks.take_own,
                callbacks.current,
                callbacks.query,
                callbacks.moved,
                callbacks.commit,
            ) else {
                return false;
            };
            let Some(_owner) = IUnknown::from_raw_borrowed(&callbacks.lifetime).cloned() else {
                return false;
            };
            let Some(context) = ITfContext::from_raw_borrowed(&context).cloned() else {
                return false;
            };
            let Some(record) = ITfEditRecord::from_raw_borrowed(&record).cloned() else {
                return false;
            };
            if take(callbacks.owner) != 0 {
                return true;
            }
            let Ok(changed) = record.GetSelectionStatus() else {
                return true;
            };
            let mut composition_context = ptr::null_mut();
            if current(callbacks.owner, &mut composition_context).is_null() {
                query(
                    callbacks.owner,
                    cookie,
                    context.as_raw(),
                    i32::from(changed.as_bool()),
                );
            }
            if !changed.as_bool() {
                return true;
            }
            let pointer = current(callbacks.owner, &mut composition_context);
            if pointer.is_null() || composition_context != context.as_raw() {
                let mouse = (GetAsyncKeyState(VK_LBUTTON.0 as i32)
                    | GetAsyncKeyState(VK_RBUTTON.0 as i32)) as u16
                    & 0x8000
                    != 0;
                moved(callbacks.owner, i32::from(mouse));
                return true;
            }
            let Some(composition) = ITfComposition::from_raw_borrowed(&pointer).cloned() else {
                return true;
            };
            let Some(selection) = ranges::selection(&context, cookie) else {
                return true;
            };
            let mut raw = ptr::null_mut();
            let status = (Interface::vtable(&composition).GetRange)(composition.as_raw(), &mut raw);
            let range = (!raw.is_null()).then(|| ITfRange::from_raw(raw));
            if status.is_err() {
                return true;
            }
            let Some(range) = range else {
                return true;
            };
            let (Ok(start), Ok(end)) = (
                selection.CompareStart(cookie, &range, TF_ANCHOR_START),
                selection.CompareEnd(cookie, &range, TF_ANCHOR_END),
            ) else {
                return true;
            };
            if start < 0 || end > 0 {
                // Range queries can synchronously terminate or replace composition.
                let active = current(callbacks.owner, &mut composition_context);
                if active != composition.as_raw() || composition_context != context.as_raw() {
                    return true;
                }
                moved(callbacks.owner, 1);
                commit(callbacks.owner);
            }
            true
        }))
        .unwrap_or(false),
    )
}
