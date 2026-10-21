// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Ordered reply application, guarded-edit failure recovery and preedit cleanup.
use super::{commits, ranges};
use serde_json::{json, Value};
use std::{
    ffi::{c_char, c_void, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};
use windows::{
    core::{IUnknown, Interface},
    Win32::UI::TextServices::*,
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Callbacks {
    pub(super) owner: *mut c_void,
    pub(super) lifetime: *mut c_void,
    pub(super) current: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub(super) write: unsafe extern "C" fn(*mut c_void, *mut c_void, *const u16, u32) -> i32,
    pub(super) action: unsafe extern "C" fn(*mut c_void, u32, u32, *mut c_void, *const c_char, i32),
}
/// # Safety
/// TSF-thread-only borrowed context, valid read/write cookie and UTF-8 reply
/// {active:bool,commits:array,view:object|null} plus {sid:string,direct:bool} metadata.
/// Non-null callbacks never throw. current returns a borrowed composition and
/// write borrows range/text. action operations: 0 set converting(value),1 update
/// composition(view JSON),2 show candidates(view JSON),3 end composition,
/// 4 hide candidates,5 guarded failure(reset JSON; mark failed/unconsumed).
/// JSON pointers are borrowed for the duration of each callback only. Owner and
/// context are retained across callbacks; the erased range is retained as well.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_apply_reply(
    context: *mut c_void,
    cookie: u32,
    reply: *const c_char,
    metadata: *const c_char,
    callbacks: *const Callbacks,
) -> i32 {
    i32::from(
        catch_unwind(AssertUnwindSafe(|| {
            let Some(c) = callbacks.as_ref().copied() else {
                return false;
            };
            let Some(_owner) = IUnknown::from_raw_borrowed(&c.lifetime).cloned() else {
                return false;
            };
            let Some(context) = ITfContext::from_raw_borrowed(&context).cloned() else {
                return false;
            };
            if reply.is_null() || metadata.is_null() {
                return false;
            }
            let Ok(reply) = serde_json::from_slice::<Value>(CStr::from_ptr(reply).to_bytes())
            else {
                return false;
            };
            let Ok(metadata) = serde_json::from_slice::<Value>(CStr::from_ptr(metadata).to_bytes())
            else {
                return false;
            };
            if !metadata["sid"].is_string() {
                return false;
            }
            let edits = reply
                .get("commits")
                .filter(|v| v.is_array())
                .cloned()
                .unwrap_or_else(|| json!([]));
            let Ok(edits) = CString::new(edits.to_string()) else {
                return false;
            };
            // A reconversion starts only over the exact selected UTF-16 text,
            // before any commits, preedit or candidate UI can be published.
            let selected = match reply.get("selectionExpect") {
                None => true,
                Some(expected) => expected
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .is_some_and(|text| {
                        let expected: Vec<_> = text.encode_utf16().collect();
                        expected.len() <= 2048
                            && (c.current)(c.owner).is_null()
                            && ranges::selection(&context, cookie)
                                .is_some_and(|r| ranges::matches_text(&r, cookie, &expected))
                    }),
            };
            if !selected
                || commits::meltype_tsf_apply_commits(
                    context.as_raw(),
                    cookie,
                    edits.as_ptr(),
                    c.owner,
                    c.current,
                    c.write,
                ) == 0
            {
                let Ok(reset) = CString::new(
                    json!({"op":"reset","sid":metadata["sid"],"direct":metadata["direct"]==true})
                        .to_string(),
                ) else {
                    return false;
                };
                (c.action)(c.owner, 5, cookie, context.as_raw(), reset.as_ptr(), 0);
                return false;
            }
            let view = &reply["view"];
            let converting = !view.is_null() && view["converting"] == true;
            (c.action)(
                c.owner,
                0,
                cookie,
                context.as_raw(),
                ptr::null(),
                i32::from(converting),
            );
            if !view.is_null() && view["text"].as_str().is_some_and(|s| !s.is_empty()) {
                let Ok(view) = CString::new(view.to_string()) else {
                    return false;
                };
                (c.action)(c.owner, 1, cookie, context.as_raw(), view.as_ptr(), 0);
                (c.action)(c.owner, 2, cookie, context.as_raw(), view.as_ptr(), 0);
                return true;
            }
            let raw = (c.current)(c.owner);
            if let Some(composition) = ITfComposition::from_raw_borrowed(&raw).cloned() {
                if reply["active"] == true {
                    let mut raw_range = ptr::null_mut();
                    let status = (Interface::vtable(&composition).GetRange)(
                        composition.as_raw(),
                        &mut raw_range,
                    );
                    let range = if raw_range.is_null() {
                        None
                    } else {
                        Some(ITfRange::from_raw(raw_range))
                    };
                    if status.is_ok() {
                        if let Some(range) = range {
                            let _ = range.SetText(cookie, 0, &[]);
                        }
                    }
                }
                if (c.current)(c.owner) == composition.as_raw() {
                    (c.action)(c.owner, 3, cookie, context.as_raw(), ptr::null(), 0);
                }
            }
            (c.action)(c.owner, 4, cookie, context.as_raw(), ptr::null(), 0);
            true
        }))
        .unwrap_or(false),
    )
}
