// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Candidate-window update order, retaining the SDK's incremental selection path.
use serde::Deserialize;
use std::ffi::c_void;
use unicode_normalization::UnicodeNormalization;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Update {
    candidates: Vec<String>,
    selected_index: i64,
    previous: Vec<String>,
    visible: bool,
    window_index: i64,
}
type Host = unsafe extern "C" fn(*mut c_void, u32, i64) -> i32;
/// Ops: begin suppression(0), replace/show/reset index(1), responds-to-direction
/// query(2: +/-1), perform direction(3), remember index(4), schedule meaning(5),
/// clear candidates/meaning and hide(6). Query returns 1=supported,0=unsupported,
/// negative=failure. Others must return positive to continue. Host ends suppression
/// even after failure. A missing SDK window must be handled before this call.
/// # Safety
/// JSON bytes and callback/context remain valid; no exception crosses the ABI.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_candidates_utf8(
    input: *const u8,
    length: u32,
    context: *mut c_void,
    host: Option<Host>,
) -> i32 {
    let Some(host) = host else {
        return 0;
    };
    if input.is_null() {
        return 0;
    }
    let Ok(update) =
        serde_json::from_slice::<Update>(std::slice::from_raw_parts(input, length as usize))
    else {
        return 0;
    };
    if update.candidates.len() <= 1 {
        return i32::from(host(context, 6, 0) > 0);
    }
    if host(context, 0, 0) <= 0 {
        return 0;
    }
    let mut index = update.window_index;
    let same = update.previous.len() == update.candidates.len()
        && update
            .previous
            .iter()
            .zip(&update.candidates)
            .all(|(a, b)| a.nfd().eq(b.nfd()));
    if !same || !update.visible {
        if host(context, 1, 0) <= 0 {
            return 0;
        }
        index = 0;
    }
    if update.selected_index >= 0 && (update.selected_index as u64) < update.candidates.len() as u64
    {
        let direction = if update.selected_index > index { 1 } else { -1 };
        let supported = host(context, 2, direction);
        if supported < 0 {
            return 0;
        }
        if supported > 0 {
            for _ in 0..update.selected_index.abs_diff(index) {
                if host(context, 3, direction) <= 0 {
                    return 0;
                }
            }
            if host(context, 4, update.selected_index) <= 0 {
                return 0;
            }
        }
    }
    i32::from(host(context, 5, 0) > 0)
}
