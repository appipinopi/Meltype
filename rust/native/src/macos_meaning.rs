// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Per-macOS-session meaning reservation. Host scheduling holds no native lock.
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{Mutex, OnceLock},
};
use unicode_normalization::UnicodeNormalization;
type Key = (i64, String);
fn states() -> &'static Mutex<HashMap<usize, Option<Key>>> {
    static STATES: OnceLock<Mutex<HashMap<usize, Option<Key>>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn attach(handle: *mut c_void) {
    if !handle.is_null() {
        states()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(handle as usize, None);
    }
}
pub fn forget(handle: *mut c_void) {
    states()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&(handle as usize));
}
unsafe fn key(index: i64, text: *const u16, length: u32) -> Option<Key> {
    if text.is_null() && length != 0 {
        return None;
    }
    let units = if length == 0 {
        &[][..]
    } else {
        std::slice::from_raw_parts(text, length as usize)
    };
    Some((index, String::from_utf16(units).ok()?.nfd().collect()))
}
/// Return 1 when a changed non-null meaning should be scheduled. present=0
/// clears the current reservation, returning 0. Empty present text is meaningful.
/// # Safety
/// text is readable for length UTF-16 units, or null for empty/absent text.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_meaning_update(
    handle: *mut c_void,
    index: i64,
    text: *const u16,
    length: u32,
    present: i32,
) -> i32 {
    let key = if present == 0 {
        None
    } else {
        let Some(key) = key(index, text, length) else {
            return 0;
        };
        Some(key)
    };
    let mut all = states().lock().unwrap_or_else(|e| e.into_inner());
    let Some(current) = all.get_mut(&(handle as usize)) else {
        return 0;
    };
    if *current == key {
        return 0;
    }
    *current = key;
    i32::from(current.is_some())
}
/// Match a timer's original key against the live reservation. Mirrors Swift's
/// canonical String equality, including A->B->A matching an original A callback.
/// Destroyed/recreated sessions cannot match a timer from an old handle.
/// # Safety
/// text follows the same UTF-16 buffer contract as meaning_update.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_meaning_matches(
    handle: *mut c_void,
    index: i64,
    text: *const u16,
    length: u32,
) -> i32 {
    let Some(key) = key(index, text, length) else {
        return 0;
    };
    i32::from(
        states()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(handle as usize))
            .is_some_and(|current| current.as_ref() == Some(&key)),
    )
}

#[no_mangle]
pub extern "C" fn meltype_macos_meaning_delay_ms() -> u32 {
    1500
}
