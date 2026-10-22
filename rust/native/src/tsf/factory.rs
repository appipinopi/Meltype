// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! COM class factory. Only construction of the not-yet-ported text service
//! crosses into the shell. Each factory pins that shell until its last Release.
use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{fence, AtomicU32, Ordering},
};
use windows::{
    core::{IUnknown, IUnknown_Vtbl, Interface, GUID, HRESULT},
    Win32::{
        Foundation::*,
        System::Com::{IClassFactory, IClassFactory_Vtbl},
    },
};

type Create = unsafe extern "C" fn(*const GUID, *mut *mut c_void) -> HRESULT;
type Lock = unsafe extern "C" fn(i32);
#[repr(C)]
struct Factory {
    vtable: *const IClassFactory_Vtbl,
    refs: AtomicU32,
    create: Create,
    lock: Lock,
}
use super::identity::CLSID;
unsafe fn factory<'a>(this: *mut c_void) -> &'a Factory {
    &*this.cast::<Factory>()
}
unsafe extern "system" fn query(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    out.write(ptr::null_mut());
    if iid.is_null() {
        return E_INVALIDARG;
    }
    if *iid != IUnknown::IID && *iid != IClassFactory::IID {
        return E_NOINTERFACE;
    }
    add_ref(this);
    out.write(this);
    S_OK
}
unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    factory(this).refs.fetch_add(1, Ordering::Relaxed) + 1
}
unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let count = factory(this).refs.fetch_sub(1, Ordering::Release) - 1;
    if count == 0 {
        fence(Ordering::Acquire);
        let lock = factory(this).lock;
        drop(Box::from_raw(this.cast::<Factory>()));
        // No shell callback or data can be used after releasing this pin.
        lock(0);
    }
    count
}
unsafe extern "system" fn create(
    this: *mut c_void,
    outer: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    out.write(ptr::null_mut());
    if iid.is_null() {
        return E_INVALIDARG;
    }
    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }
    (factory(this).create)(iid, out)
}
unsafe extern "system" fn lock(this: *mut c_void, value: BOOL) -> HRESULT {
    (factory(this).lock)(i32::from(value.as_bool()));
    S_OK
}
static VTABLE: IClassFactory_Vtbl = IClassFactory_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: query,
        AddRef: add_ref,
        Release: release,
    },
    CreateInstance: create,
    LockServer: lock,
};

/// Returns an owned IUnknown/IClassFactory. `lock(1)` pins shell callback code;
/// `lock(0)` releases it. LockServer holds independent pins until balanced.
/// # Safety
/// GUID pointers must be readable and `out` writable. Callbacks must remain
/// callable across COM threads until all factory references and server locks end.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_get_class_object(
    clsid: *const GUID,
    iid: *const GUID,
    out: *mut *mut c_void,
    create: Option<Create>,
    lock: Option<Lock>,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    out.write(ptr::null_mut());
    if clsid.is_null() || iid.is_null() {
        return E_INVALIDARG;
    }
    if *clsid != CLSID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    if *iid != IUnknown::IID && *iid != IClassFactory::IID {
        return E_NOINTERFACE;
    }
    let (Some(create), Some(lock)) = (create, lock) else {
        return E_INVALIDARG;
    };
    let pointer = Box::into_raw(Box::new(Factory {
        vtable: &VTABLE,
        refs: AtomicU32::new(1),
        create,
        lock,
    }));
    lock(1);
    out.write(pointer.cast());
    S_OK
}
