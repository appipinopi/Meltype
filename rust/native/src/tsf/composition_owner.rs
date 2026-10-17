// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Ownership and lifecycle of the shell's composition/context references. Slots
//! are accessed only with raw reads/writes, never borrowed across reentrant COM.
use super::{composition, ranges};
use std::{
    ffi::{c_char, c_void},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
};
use windows::{
    core::{IUnknown, Interface},
    Win32::UI::TextServices::*,
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Slots {
    lifetime: *mut c_void,
    composition: *mut *mut c_void,
    context: *mut *mut c_void,
    sink: *mut c_void,
}
impl Slots {
    unsafe fn ensure(&self, context: &ITfContext, cookie: u32, range: *mut c_void) -> bool {
        if !self.composition.read().is_null() {
            return true;
        }
        let created =
            composition::meltype_tsf_start_composition(context.as_raw(), self.sink, range, cookie);
        if created.is_null() {
            return false;
        }
        let created = ITfComposition::from_raw(created);
        if self.composition.read().is_null() {
            // Publish both owned references before releasing an old context,
            // whose Release can call back into the service.
            let old_context = self.context.replace(context.clone().into_raw());
            self.composition.write(created.clone().into_raw());
            if !old_context.is_null() {
                drop(ITfContext::from_raw(old_context));
            }
        } else {
            // A synchronous callback installed another composition while Start
            // ran. Keep that input and end only the unused newly-created one.
            if self.composition.read() != created.as_raw() {
                composition::meltype_tsf_end_composition(
                    context.as_raw(),
                    created.as_raw(),
                    cookie,
                );
            }
            return false;
        }
        self.composition.read() == created.as_raw() && self.context.read() == context.as_raw()
    }
    unsafe fn finish(&self, cookie: u32) -> bool {
        let original = self.composition.read();
        let Some(target) = ITfComposition::from_raw_borrowed(&original).cloned() else {
            return false;
        };
        let original_context = self.context.read();
        let context = ITfContext::from_raw_borrowed(&original_context).cloned();
        composition::meltype_tsf_end_composition(
            context.as_ref().map_or(ptr::null_mut(), Interface::as_raw),
            target.as_raw(),
            cookie,
        );
        // A replacement in the same context owns its own context reference.
        // Clear the pair before Release, and never clear that replacement's slot.
        let still_old = self.composition.read() == original;
        let composition = if still_old {
            self.composition.replace(ptr::null_mut())
        } else {
            ptr::null_mut()
        };
        let context = if (still_old || self.composition.read().is_null())
            && self.context.read() == original_context
        {
            self.context.replace(ptr::null_mut())
        } else {
            ptr::null_mut()
        };
        if !composition.is_null() {
            drop(ITfComposition::from_raw(composition));
        }
        if !context.is_null() {
            drop(ITfContext::from_raw(context));
        }
        true
    }
}
unsafe fn slots(slots: *const Slots) -> Option<(Slots, IUnknown)> {
    let s = slots.as_ref().copied()?;
    if s.composition.is_null() || s.context.is_null() || s.composition == s.context {
        return None;
    }
    Some((s, IUnknown::from_raw_borrowed(&s.lifetime)?.clone()))
}
/// # Safety
/// On the TSF thread. Slots point to service-owned COM pointers and remain valid
/// while lifetime IUnknown is retained. Existing refs are owned; new refs are
/// transferred into slots. Callbacks may replace/clear slots using the same
/// ownership convention. sink is a borrowed ITfCompositionSink. JSON is a live
/// NUL-terminated UTF-8 view. Returns 0=start failed,1=update attempted,-1=invalid.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_apply_owned_composition(
    context: *mut c_void,
    cookie: u32,
    json: *const c_char,
    input: u32,
    converted: u32,
    target: u32,
    owner: *const Slots,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some((s, _owner)) = slots(owner) else {
            return -1;
        };
        let Some(context) = ITfContext::from_raw_borrowed(&context).cloned() else {
            return -1;
        };
        if json.is_null() {
            return -1;
        }
        if !s.ensure(&context, cookie, ptr::null_mut()) {
            return 0;
        }
        let raw = s.composition.read();
        let Some(composition) = ITfComposition::from_raw_borrowed(&raw).cloned() else {
            return 0;
        };
        if s.context.read() != context.as_raw() {
            return 0;
        }
        composition::meltype_tsf_update_composition(
            context.as_raw(),
            composition.as_raw(),
            cookie,
            json,
            input,
            converted,
            target,
        );
        1
    }))
    .unwrap_or(-1)
}
/// # Safety
/// Same slot/thread contract. Borrowed range and UTF-16 text valid until return.
/// Returns flags written=1,composition ending attempted=2; -1=invalid.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_write_owned_commit(
    context: *mut c_void,
    range: *mut c_void,
    cookie: u32,
    text: *const u16,
    length: u32,
    keep: i32,
    owner: *const Slots,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some((s, _owner)) = slots(owner) else {
            return -1;
        };
        let Some(context) = ITfContext::from_raw_borrowed(&context).cloned() else {
            return -1;
        };
        let Some(range) = ITfRange::from_raw_borrowed(&range).cloned() else {
            return -1;
        };
        if length != 0 && text.is_null() {
            return -1;
        }
        let prepared = s.ensure(&context, cookie, range.as_raw());
        let original = s.composition.read();
        let target = ITfComposition::from_raw_borrowed(&original).cloned();
        let written = ranges::meltype_tsf_write_range(
            context.as_raw(),
            range.as_raw(),
            cookie,
            text,
            length,
            keep,
        ) != 0;
        let ended =
            prepared && target.is_some() && s.composition.read() == original && s.finish(cookie);
        i32::from(written) | if ended { 2 } else { 0 }
    }))
    .unwrap_or(-1)
}
/// # Safety
/// Same slot/thread contract. Retains old references across notifications; clears
/// only that pair and preserves any new composition even in the same context.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_end_owned_composition(
    cookie: u32,
    owner: *const Slots,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some((s, _owner)) = slots(owner) else {
            return -1;
        };
        i32::from(s.finish(cookie))
    }))
    .unwrap_or(-1)
}
