// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! UTF-16 TSF range reads and guarded writes. The host supplies the edit cookie;
//! this module never requests a lock or falls back to keyboard injection.
use std::{
    ffi::c_void,
    mem::ManuallyDrop,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};
use windows::{core::Interface, Win32::UI::TextServices::*};
type Surrounding = unsafe extern "C" fn(*mut c_void, *const u16, u32, *const u16, u32);
pub(super) unsafe fn composition_range(composition: &ITfComposition) -> Option<ITfRange> {
    let mut raw = ptr::null_mut();
    let status = (Interface::vtable(composition).GetRange)(composition.as_raw(), &mut raw);
    let range = if raw.is_null() {
        None
    } else {
        Some(ITfRange::from_raw(raw))
    };
    if status.is_ok() {
        range
    } else {
        None
    }
}
pub(super) unsafe fn attribute_property(context: &ITfContext) -> Option<ITfProperty> {
    let mut raw = ptr::null_mut();
    let status =
        (Interface::vtable(context).GetProperty)(context.as_raw(), &GUID_PROP_ATTRIBUTE, &mut raw);
    let property = if raw.is_null() {
        None
    } else {
        Some(ITfProperty::from_raw(raw))
    };
    if status.is_ok() {
        property
    } else {
        None
    }
}
pub(super) unsafe fn selection(context: &ITfContext, cookie: u32) -> Option<ITfRange> {
    let mut value = [TF_SELECTION::default()];
    let mut fetched = 0;
    let status = context.GetSelection(cookie, TF_DEFAULT_SELECTION, &mut value, &mut fetched);
    let range = ManuallyDrop::take(&mut value[0].range);
    if status.is_err() || fetched == 0 {
        return None;
    }
    range
}
pub(super) unsafe fn caret(context: &ITfContext, range: &ITfRange, cookie: u32) {
    if let Ok(caret) = range.Clone() {
        if caret.Collapse(cookie, TF_ANCHOR_END).is_ok() {
            let mut value = [TF_SELECTION {
                range: ManuallyDrop::new(Some(caret)),
                style: TF_SELECTIONSTYLE {
                    ase: TF_AE_NONE,
                    fInterimChar: false.into(),
                },
            }];
            let _ = context.SetSelection(cookie, &value);
            drop(ManuallyDrop::take(&mut value[0].range));
        }
    }
}
fn guard(f: impl FnOnce() -> bool) -> i32 {
    i32::from(catch_unwind(AssertUnwindSafe(f)).unwrap_or(false))
}
pub(super) unsafe fn read(range: &ITfRange, cookie: u32, limit: usize) -> Option<Vec<u16>> {
    let mut text = vec![0; limit];
    let mut count = 0;
    range.GetText(cookie, 0, &mut text, &mut count).ok()?;
    if count as usize > limit {
        return None;
    }
    text.truncate(count as usize);
    Some(text)
}
/// # Safety
/// `context` is a borrowed ITfContext. Callback buffers are valid only during the
/// callback; use on the TSF thread with a valid read/write cookie.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_read_surrounding(
    context: *mut c_void,
    cookie: u32,
    receiver: *mut c_void,
    callback: Surrounding,
) -> i32 {
    guard(|| {
        let Some(context) = ITfContext::from_raw_borrowed(&context) else {
            return false;
        };
        let Some(range) = selection(context, cookie) else {
            return false;
        };
        let before = (|| {
            let value = range.Clone().ok()?;
            value.Collapse(cookie, TF_ANCHOR_START).ok()?;
            let mut moved = 0;
            value
                .ShiftStart(cookie, -4000, &mut moved, ptr::null())
                .ok()?;
            read(&value, cookie, 4000)
        })()
        .unwrap_or_default();
        let after = (|| {
            let value = range.Clone().ok()?;
            value.Collapse(cookie, TF_ANCHOR_END).ok()?;
            let mut moved = 0;
            value.ShiftEnd(cookie, 20, &mut moved, ptr::null()).ok()?;
            read(&value, cookie, 20)
        })()
        .unwrap_or_default();
        callback(
            receiver,
            before.as_ptr(),
            before.len() as u32,
            after.as_ptr(),
            after.len() as u32,
        );
        true
    })
}
/// Extend a collapsed range only when the exact UTF-16 prefix was read back.
/// # Safety
/// `range` is a borrowed ITfRange and `expected` points to `length` UTF-16 units.
/// No text is modified here. A failed final shift may change the range boundary;
/// the caller must discard that range and must not write on failure.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_extend_previous(
    range: *mut c_void,
    cookie: u32,
    count: i32,
    expected: *const u16,
    length: u32,
) -> i32 {
    guard(|| {
        if expected.is_null() || count <= 0 || count as u32 != length || length > 1_048_576 {
            return false;
        }
        let Some(range) = ITfRange::from_raw_borrowed(&range) else {
            return false;
        };
        if !range.IsEmpty(cookie).is_ok_and(|value| value.as_bool()) {
            return false;
        }
        let Ok(previous) = range.Clone() else {
            return false;
        };
        if previous.Collapse(cookie, TF_ANCHOR_START).is_err() {
            return false;
        }
        let mut moved = 0;
        if previous
            .ShiftStart(cookie, -count, &mut moved, ptr::null())
            .is_err()
            || moved.unsigned_abs() != length
        {
            return false;
        }
        let Some(text) = read(&previous, cookie, length as usize + 1) else {
            return false;
        };
        if text != std::slice::from_raw_parts(expected, length as usize) {
            return false;
        }
        range
            .ShiftStart(cookie, -count, &mut moved, ptr::null())
            .is_ok()
            && moved.unsigned_abs() == length
    })
}
/// Clear composition attributes, write the text, and optionally move the caret.
/// # Safety
/// Borrowed context/range and valid edit cookie. `text` points to `length` UTF-16
/// units (may be null only for zero length). Caller owns composition start/end.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_write_range(
    context: *mut c_void,
    range: *mut c_void,
    cookie: u32,
    text: *const u16,
    length: u32,
    keep_caret: i32,
) -> i32 {
    guard(|| {
        if (text.is_null() && length != 0) || length > i32::MAX as u32 {
            return false;
        }
        let Some(context) = ITfContext::from_raw_borrowed(&context) else {
            return false;
        };
        let Some(range) = ITfRange::from_raw_borrowed(&range) else {
            return false;
        };
        if let Some(attribute) = attribute_property(context) {
            let _ = attribute.Clear(cookie, range);
        }
        let text = if length == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(text, length as usize)
        };
        if range.SetText(cookie, 0, text).is_err() {
            return false;
        }
        if keep_caret == 0 {
            caret(context, range, cookie);
        }
        true
    })
}
