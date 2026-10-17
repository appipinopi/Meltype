// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Complete native service owner, state allocation and COM construction.
use super::*;
use crate::tsf::service_com::{self, Call};
use windows::{core::GUID, Win32::Foundation::E_OUTOFMEMORY};
type Notice = unsafe extern "C" fn(*mut c_void, u32, i32);
type Lock = unsafe extern "C" fn(i32);
struct Object {
    state: *mut ServiceState,
    root: *mut c_void,
    worker: CString,
    adapter: *mut c_void,
    mode: Option<Notice>,
    key: Option<Notice>,
    activation: Option<Notice>,
    shutdown: Option<Notice>,
    lock: Option<Lock>,
}
impl Drop for Object {
    fn drop(&mut self) {
        unsafe {
            meltype_tsf_service_state_destroy(self.state);
            // Retire UI, transport and owned native state before the shell pin.
            if let Some(lock) = self.lock {
                lock(0);
            }
        }
    }
}
unsafe extern "C" fn dispatch(data: *mut c_void, call: *const Call) -> HRESULT {
    let o = &*data.cast::<Object>();
    super::route::meltype_tsf_service_route(
        o.state,
        o.root,
        o.worker.as_ptr(),
        call,
        o.adapter,
        o.mode,
        o.key,
        o.activation,
        o.shutdown,
    )
}
unsafe extern "C" fn destroy(data: *mut c_void) {
    drop(Box::from_raw(data.cast::<Object>()));
}
/// Create a requested service interface with native owner/state/COM storage.
/// # Safety
/// Called on its TSF thread; iid readable and out writable. Worker trusted absolute
/// UTF8 and copied. Optional lock pins external notice code through final native
/// state teardown; callbacks never unwind. Adapter/notices remain live until
/// final Release. OS must Deactivate before releasing active service subscriptions.
/// Failure clears out and balances state/pin. Returns one owned COM reference.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_object_create(
    worker: *const c_char,
    iid: *const GUID,
    out: *mut *mut c_void,
    adapter: *mut c_void,
    mode: Option<Notice>,
    key: Option<Notice>,
    activation: Option<Notice>,
    shutdown: Option<Notice>,
    lock: Option<Lock>,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    out.write(ptr::null_mut());
    if worker.is_null() || iid.is_null() {
        return E_INVALIDARG;
    }
    let worker = CStr::from_ptr(worker).to_owned();
    let state = meltype_tsf_service_state_create();
    if state.is_null() {
        return E_OUTOFMEMORY;
    }
    if let Some(lock) = lock {
        lock(1);
    }
    (*state).keys = key_state::meltype_tsf_key_state_create();
    let object = Box::new(Object {
        state,
        root: ptr::null_mut(),
        worker,
        adapter,
        mode,
        key,
        activation,
        shutdown,
        lock,
    });
    if (*state).keys.is_null() {
        drop(object);
        return E_OUTOFMEMORY;
    }
    let data = Box::into_raw(object);
    let root =
        service_com::meltype_tsf_service_com_create(data.cast(), Some(dispatch), Some(destroy));
    if root.is_null() {
        drop(Box::from_raw(data));
        return E_OUTOFMEMORY;
    }
    (*data).root = root;
    let unknown = IUnknown::from_raw(root);
    (Interface::vtable(&unknown).QueryInterface)(unknown.as_raw(), iid, out)
}
