// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Previously focused compositions, retained until editing becomes possible.
use super::edit_session;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};
use windows::{
    core::{IUnknown, Interface, HRESULT},
    Win32::{Foundation::*, UI::TextServices::*},
};
struct Item {
    composition: ITfComposition,
    context: ITfContext,
    ended: Cell<bool>,
}
#[derive(Default)]
struct State {
    items: Vec<Rc<Item>>,
    deactivated: bool,
}
type Shared = Rc<RefCell<State>>;
thread_local! { static STATES:RefCell<HashMap<usize,Shared>>=RefCell::new(HashMap::new()); }
static NEXT: AtomicUsize = AtomicUsize::new(1);
fn state(handle: *mut c_void) -> Option<Shared> {
    STATES
        .try_with(|m| {
            m.try_borrow()
                .ok()
                .and_then(|m| m.get(&(handle as usize)).cloned())
        })
        .ok()
        .flatten()
}
unsafe fn same(a: &ITfComposition, b: &ITfComposition) -> bool {
    a.as_raw() == b.as_raw()
        || a.cast::<IUnknown>()
            .ok()
            .zip(b.cast::<IUnknown>().ok())
            .is_some_and(|(a, b)| a == b)
}
unsafe fn forget(state: &Shared, composition: &ITfComposition) -> Option<Rc<Item>> {
    // COM QueryInterface/Release can call back into the owner: never hold the
    // state borrow across COM. Match a retained snapshot, then remove by pointer.
    let snapshot = state.borrow().items.clone();
    let matched = snapshot
        .iter()
        .find(|item| same(&item.composition, composition));
    matched.and_then(|item| {
        let mut value = state.borrow_mut();
        let index = value
            .items
            .iter()
            .position(|entry| entry.composition.as_raw() == item.composition.as_raw())?;
        Some(value.items.remove(index))
    })
}
unsafe fn clear(composition: &ITfComposition, cookie: u32) {
    if let Ok(range) = composition.GetRange() {
        if let Ok(context) = range.GetContext() {
            if let Ok(property) = context.GetProperty(&GUID_PROP_ATTRIBUTE) {
                let _ = property.Clear(cookie, &range);
            }
        }
    }
}
struct Pending {
    state: Option<Shared>,
    item: Rc<Item>,
    _owner: IUnknown,
}
unsafe extern "C" fn run(data: *mut c_void, cookie: u32) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let value = &*data.cast::<Pending>();
        let end = match &value.state {
            Some(state) => {
                forget(state, &value.item.composition).is_some() || state.borrow().deactivated
            }
            None => true,
        };
        if end {
            // The same item can have an older asynchronous request and a newer
            // synchronous deactivation request. End only once, even reentrantly.
            if !value.item.ended.replace(true) {
                clear(&value.item.composition, cookie);
                if value.item.composition.EndComposition(cookie).is_err() {
                    value.item.ended.set(false);
                }
            }
        }
        S_OK
    }))
    .unwrap_or(E_FAIL)
}
unsafe extern "C" fn destroy(data: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        drop(Box::from_raw(data.cast::<Pending>()))
    }));
}
unsafe fn submit(
    state: Option<Shared>,
    item: Rc<Item>,
    owner: IUnknown,
    client: u32,
    flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS,
) -> HRESULT {
    let context = item.context.clone();
    let action = Box::into_raw(Box::new(Pending {
        state,
        item,
        _owner: owner,
    }));
    edit_session::meltype_tsf_request_edit_session(
        context.as_raw(),
        client,
        flags.0,
        action.cast(),
        Some(run),
        destroy,
    )
}
#[no_mangle]
pub extern "C" fn meltype_tsf_abandoned_create() -> *mut c_void {
    catch_unwind(|| {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        STATES.with(|m| {
            m.borrow_mut()
                .insert(id, Rc::new(RefCell::new(State::default())))
        });
        id as *mut c_void
    })
    .unwrap_or(ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn meltype_tsf_abandoned_reset(handle: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(state) = state(handle) {
            let old = std::mem::take(&mut *state.borrow_mut());
            drop(old);
        }
    }));
}
/// Borrowed interfaces, on their creating TSF thread. Pending requests retain
/// the owner, composition and context until execution or cancellation.
/// # Safety
/// Non-null arguments must be live interfaces of the indicated types.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_abandoned_add(
    handle: *mut c_void,
    context: *mut c_void,
    composition: *mut c_void,
    owner: *mut c_void,
    client: u32,
) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(state) = state(handle) else {
            return E_INVALIDARG;
        };
        let (Some(context), Some(composition), Some(owner)) = (
            ITfContext::from_raw_borrowed(&context),
            ITfComposition::from_raw_borrowed(&composition),
            IUnknown::from_raw_borrowed(&owner),
        ) else {
            return E_INVALIDARG;
        };
        let item = Rc::new(Item {
            context: context.clone(),
            composition: composition.clone(),
            ended: Cell::new(false),
        });
        let removed = {
            let mut value = state.borrow_mut();
            let old = if value.items.len() >= 8 {
                Some(value.items.remove(0))
            } else {
                None
            };
            value.items.push(item.clone());
            old
        };
        drop(removed);
        submit(
            Some(state),
            item,
            owner.clone(),
            client,
            TF_ES_ASYNCDONTCARE | TF_ES_READWRITE,
        )
    }))
    .unwrap_or(E_FAIL)
}
/// # Safety
/// Borrowed composition on the token's creating TSF thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_abandoned_forget(
    handle: *mut c_void,
    composition: *mut c_void,
) -> i32 {
    i32::from(
        catch_unwind(AssertUnwindSafe(|| {
            let (Some(state), Some(composition)) = (
                state(handle),
                ITfComposition::from_raw_borrowed(&composition),
            ) else {
                return false;
            };
            if let Some(item) = forget(&state, composition) {
                item.ended.set(true);
                true
            } else {
                false
            }
        }))
        .unwrap_or(false),
    )
}
/// Deactivate, drain retained items and attempt synchronous cleanup. Queued
/// requests keep the inactive state alive and may finish after refusal here.
/// # Safety
/// Owner is a borrowed live IUnknown on the token's TSF thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_abandoned_end(
    handle: *mut c_void,
    owner: *mut c_void,
    client: u32,
) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let (Some(state), Some(owner)) = (state(handle), IUnknown::from_raw_borrowed(&owner))
        else {
            return E_INVALIDARG;
        };
        let items = {
            let mut value = state.borrow_mut();
            value.deactivated = true;
            std::mem::take(&mut value.items)
        };
        let mut result = S_OK;
        for item in items {
            let hr = submit(
                None,
                item,
                owner.clone(),
                client,
                TF_ES_SYNC | TF_ES_READWRITE,
            );
            if hr.is_err() && result.is_ok() {
                result = hr;
            }
        }
        result
    }))
    .unwrap_or(E_FAIL)
}
#[no_mangle]
pub extern "C" fn meltype_tsf_abandoned_destroy(handle: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let state = STATES
            .try_with(|m| {
                m.try_borrow_mut()
                    .ok()
                    .and_then(|mut m| m.remove(&(handle as usize)))
            })
            .ok()
            .flatten();
        if let Some(state) = state {
            let items = {
                let mut value = state.borrow_mut();
                value.deactivated = true;
                std::mem::take(&mut value.items)
            };
            drop(items);
        }
    }));
}
/// # Safety
/// Borrowed live composition and valid write cookie on its TSF thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_clear_composition_attribute(
    composition: *mut c_void,
    cookie: u32,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(composition) = ITfComposition::from_raw_borrowed(&composition).cloned() {
            clear(&composition, cookie);
        }
    }));
}
