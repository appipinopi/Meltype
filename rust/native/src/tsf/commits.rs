// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Apply commit batches in order, anchored after the preceding edit rather than
//! at an application caret that may have moved before a deferred callback runs.
use super::ranges;
use serde::Deserialize;
use std::{
    ffi::{c_char, c_void, CStr},
    panic::{catch_unwind, AssertUnwindSafe},
};
use windows::{core::Interface, Win32::UI::TextServices::*};
type Current = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type Write = unsafe extern "C" fn(*mut c_void, *mut c_void, *const u16, u32) -> i32;
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Edit {
    text: String,
    caret_back: u32,
    delete_before: i32,
    expect: Option<String>,
}
/// # Safety
/// Borrowed context, valid edit cookie and UTF-8 JSON array of commits. Owner and
/// callbacks must remain valid during this synchronous call. Current returns a
/// borrowed composition; write borrows the range/text only during its callback.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_apply_commits(
    context: *mut c_void,
    cookie: u32,
    json: *const c_char,
    owner: *mut c_void,
    current: Current,
    write: Write,
) -> i32 {
    i32::from(
        catch_unwind(AssertUnwindSafe(|| {
            if json.is_null() {
                return false;
            }
            let Some(context) = ITfContext::from_raw_borrowed(&context) else {
                return false;
            };
            let context = context.clone();
            let Ok(edits) = serde_json::from_slice::<Vec<Edit>>(CStr::from_ptr(json).to_bytes())
            else {
                return false;
            };
            // Reject arbitrary backward movement before making any edits.
            if edits.iter().any(|edit| {
                edit.caret_back != 0
                    && (edit.caret_back != 1
                        || edit.delete_before != 0
                        || !meltype_core::shortcuts::is_bracket_pair(&edit.text))
            }) {
                return false;
            }
            let mut next: Option<ITfRange> = None;
            for edit in edits {
                let composition = current(owner);
                let composition = ITfComposition::from_raw_borrowed(&composition).cloned();
                // Never extend a correction outside an active composition.
                if edit.delete_before > 0 && composition.is_some() {
                    continue;
                }
                let range = if let Some(composition) = &composition {
                    composition.GetRange().ok()
                } else if let Some(next) = &next {
                    next.Clone().ok()
                } else {
                    ranges::selection(&context, cookie)
                };
                let Some(range) = range else {
                    if edit.delete_before > 0 {
                        return false;
                    }
                    continue;
                };
                if edit.delete_before > 0 {
                    let Some(expected) = edit.expect else {
                        return false;
                    };
                    let expected: Vec<_> = expected.encode_utf16().collect();
                    if ranges::meltype_tsf_extend_previous(
                        range.as_raw(),
                        cookie,
                        edit.delete_before,
                        expected.as_ptr(),
                        expected.len() as u32,
                    ) == 0
                    {
                        return false;
                    }
                }
                let text: Vec<_> = edit.text.encode_utf16().collect();
                if text.len() > i32::MAX as usize {
                    return false;
                }
                if write(owner, range.as_raw(), text.as_ptr(), text.len() as u32) == 0 {
                    if edit.delete_before > 0 {
                        return false;
                    }
                    continue;
                }
                next = range.Clone().ok();
                if let Some(next) = &next {
                    let _ = next.Collapse(cookie, TF_ANCHOR_END);
                    if edit.caret_back == 1 {
                        let mut shifted = 0;
                        if next
                            .ShiftStart(cookie, -1, &mut shifted, std::ptr::null())
                            .is_err()
                            || shifted != -1
                        {
                            return false;
                        }
                        if next.Collapse(cookie, TF_ANCHOR_START).is_err() {
                            return false;
                        }
                        ranges::caret(&context, next, cookie);
                    }
                }
            }
            true
        }))
        .unwrap_or(false),
    )
}
