// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Recover a host-committed preedit without inserting it again. Snapshots own
//! their COM ranges/contexts and can only be consumed on their creating thread.
use super::{composition, ranges};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};
use windows::{core::Interface, Win32::UI::TextServices::*};
struct Snapshot {
    context: ITfContext,
    range: ITfRange,
    text: Vec<u16>,
}
thread_local! { static SNAPSHOTS:RefCell<HashMap<usize,Snapshot>>=RefCell::new(HashMap::new()); }
static NEXT: AtomicUsize = AtomicUsize::new(1);
fn take(id: usize) -> Option<Snapshot> {
    SNAPSHOTS
        .try_with(|m| m.try_borrow_mut().ok().and_then(|mut m| m.remove(&id)))
        .ok()
        .flatten()
}
/// Return a thread-affine snapshot token for a nonempty preedit shorter than 64
/// UTF-16 units. Capture is called only for eligible, nonconverting input.
/// # Safety
/// Borrowed context/composition and valid read cookie on the TSF thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_capture_resume(
    context: *mut c_void,
    composition: *mut c_void,
    cookie: u32,
) -> *mut c_void {
    catch_unwind(AssertUnwindSafe(|| {
        let context = ITfContext::from_raw_borrowed(&context)?.clone();
        let composition = ITfComposition::from_raw_borrowed(&composition)?;
        let range = composition.GetRange().ok()?.Clone().ok()?;
        let text = ranges::read(&range, cookie, 64)?;
        if text.is_empty() || text.len() >= 64 {
            return None;
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        SNAPSHOTS.with(|m| {
            m.borrow_mut().insert(
                id,
                Snapshot {
                    context,
                    range,
                    text,
                },
            )
        });
        Some(id as *mut c_void)
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}
/// Compares the saved input context pointer, without querying another field.
#[no_mangle]
pub extern "C" fn meltype_tsf_resume_matches(handle: *mut c_void, context: *mut c_void) -> i32 {
    i32::from(
        SNAPSHOTS
            .try_with(|m| {
                m.try_borrow().ok().is_some_and(|m| {
                    m.get(&(handle as usize))
                        .is_some_and(|s| s.context.as_raw() == context)
                })
            })
            .unwrap_or(false),
    )
}
/// Consume a snapshot even on failure; never insert/replace document text.
/// Success returns an owned ITfComposition reference, released with COM Release.
/// # Safety
/// Borrowed current context/sink, valid write cookie, on the snapshot's thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_try_resume(
    handle: *mut c_void,
    context: *mut c_void,
    sink: *mut c_void,
    cookie: u32,
) -> *mut c_void {
    catch_unwind(AssertUnwindSafe(|| {
        let saved = take(handle as usize)?;
        if saved.context.as_raw() != context {
            return None;
        }
        if ranges::read(&saved.range, cookie, 64)? != saved.text {
            return None;
        }
        let selected = ranges::selection(&saved.context, cookie)?;
        if selected
            .CompareStart(cookie, &saved.range, TF_ANCHOR_END)
            .ok()?
            != 0
            || selected
                .CompareEnd(cookie, &saved.range, TF_ANCHOR_END)
                .ok()?
                != 0
        {
            return None;
        }
        let result =
            composition::meltype_tsf_start_composition(context, sink, saved.range.as_raw(), cookie);
        if result.is_null() {
            return None;
        }
        Some(result)
    }))
    .ok()
    .flatten()
    .unwrap_or(ptr::null_mut())
}
/// Style after the shell adopts the returned composition, so a synchronous
/// termination notification can find and release its actual owner.
/// # Safety
/// Borrowed context/composition and valid write cookie on their TSF thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_resume_attribute(
    context: *mut c_void,
    composition: *mut c_void,
    cookie: u32,
    atom: u32,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let Some(context) = ITfContext::from_raw_borrowed(&context).cloned() else {
            return;
        };
        let Some(composition) = ITfComposition::from_raw_borrowed(&composition).cloned() else {
            return;
        };
        if let Ok(range) = composition.GetRange() {
            composition::meltype_tsf_set_attribute(context.as_raw(), range.as_raw(), cookie, atom);
        }
    }));
}
/// Release an unused snapshot on its creating thread. Stale/foreign tokens are
/// ignored; COM references are dropped outside the map borrow for reentrancy.
#[no_mangle]
pub extern "C" fn meltype_tsf_destroy_resume(handle: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| drop(take(handle as usize))));
}
