// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! TSF composition lifecycle and UTF-16 clause styling.
use super::ranges;
use serde::Deserialize;
use std::{
    ffi::{c_char, c_void, CStr},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};
use windows::{
    core::{Interface, VARIANT},
    Win32::UI::TextServices::*,
};
fn guard(f: impl FnOnce() -> bool) -> i32 {
    i32::from(catch_unwind(AssertUnwindSafe(f)).unwrap_or(false))
}
unsafe fn attribute(context: &ITfContext, range: &ITfRange, cookie: u32, atom: u32) {
    // msctf.h defines TF_INVALID_GUIDATOM as zero (not emitted in Win32 metadata).
    if atom == 0 {
        return;
    }
    if let Ok(property) = context.GetProperty(&GUID_PROP_ATTRIBUTE) {
        let value = VARIANT::from(atom as i32);
        let _ = property.SetValue(cookie, range, &value);
    }
}
/// Return an owned ITfComposition reference (COM Release, not meltype_free).
/// # Safety
/// Borrowed context/sink/optional range; valid cookie on the TSF thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_start_composition(
    context: *mut c_void,
    sink: *mut c_void,
    range: *mut c_void,
    cookie: u32,
) -> *mut c_void {
    catch_unwind(AssertUnwindSafe(|| {
        let context = ITfContext::from_raw_borrowed(&context)?;
        let sink = ITfCompositionSink::from_raw_borrowed(&sink)?;
        let range = if range.is_null() {
            ranges::selection(context, cookie)?
        } else {
            ITfRange::from_raw_borrowed(&range)?.clone()
        };
        let manager = context.cast::<ITfContextComposition>().ok()?;
        manager
            .StartComposition(cookie, &range, sink)
            .ok()
            .map(Interface::into_raw)
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct View {
    text: String,
    converting: bool,
    clauses: Vec<String>,
    selected_clause: i32,
}
/// # Safety
/// Borrowed context/composition, valid cookie and NUL-terminated UTF-8 view JSON.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_update_composition(
    context: *mut c_void,
    composition: *mut c_void,
    cookie: u32,
    json: *const c_char,
    input: u32,
    converted: u32,
    target: u32,
) -> i32 {
    guard(|| {
        if json.is_null() {
            return false;
        }
        let Some(context) = ITfContext::from_raw_borrowed(&context) else {
            return false;
        };
        let Some(composition) = ITfComposition::from_raw_borrowed(&composition) else {
            return false;
        };
        let context = context.clone();
        let composition = composition.clone();
        let Ok(view) = serde_json::from_slice::<View>(CStr::from_ptr(json).to_bytes()) else {
            return false;
        };
        let text: Vec<_> = view.text.encode_utf16().collect();
        if text.len() > i32::MAX as usize {
            return false;
        }
        let lengths: Vec<_> = view
            .clauses
            .iter()
            .map(|s| s.encode_utf16().count())
            .collect();
        if view.converting && !lengths.is_empty() && lengths.iter().sum::<usize>() != text.len() {
            return false;
        }
        let Ok(range) = composition.GetRange() else {
            return false;
        };
        if range.SetText(cookie, 0, &text).is_err() {
            return false;
        }
        if view.converting && !lengths.is_empty() {
            let mut offset = 0;
            for (index, length) in lengths.iter().enumerate() {
                if let Ok(clause) = range.Clone() {
                    let mut moved = 0;
                    if clause.Collapse(cookie, TF_ANCHOR_START).is_ok()
                        && clause
                            .ShiftEnd(cookie, (offset + length) as i32, &mut moved, ptr::null())
                            .is_ok()
                        && moved.unsigned_abs() as usize == offset + length
                        && clause
                            .ShiftStart(cookie, offset as i32, &mut moved, ptr::null())
                            .is_ok()
                        && moved.unsigned_abs() as usize == offset
                    {
                        attribute(
                            &context,
                            &clause,
                            cookie,
                            if index as i32 == view.selected_clause {
                                target
                            } else {
                                converted
                            },
                        );
                    }
                }
                offset += length;
            }
        } else {
            attribute(&context, &range, cookie, input);
        }
        ranges::caret(&context, &range, cookie);
        true
    })
}
/// # Safety
/// Borrowed TSF context/composition pointers and valid edit cookie. The caller
/// retains its references; this call holds extra references across callbacks.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_end_composition(
    context: *mut c_void,
    composition: *mut c_void,
    cookie: u32,
) -> i32 {
    guard(|| {
        let Some(composition) = ITfComposition::from_raw_borrowed(&composition) else {
            return false;
        };
        let composition = composition.clone();
        let context = ITfContext::from_raw_borrowed(&context).cloned();
        if let (Some(context), Ok(range)) = (&context, composition.GetRange()) {
            if let Ok(property) = context.GetProperty(&GUID_PROP_ATTRIBUTE) {
                let _ = property.Clear(cookie, &range);
            }
        }
        composition.EndComposition(cookie).is_ok()
    })
}
/// # Safety
/// Borrowed context/range pointers and valid edit cookie.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_set_attribute(
    context: *mut c_void,
    range: *mut c_void,
    cookie: u32,
    atom: u32,
) {
    guard(|| {
        let Some(context) = ITfContext::from_raw_borrowed(&context) else {
            return false;
        };
        let Some(range) = ITfRange::from_raw_borrowed(&range) else {
            return false;
        };
        attribute(context, range, cookie, atom);
        true
    });
}
