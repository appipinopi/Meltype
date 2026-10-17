// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! TSF edit-session COM ownership, including deferred callbacks retained by
//! ITfContext after RequestEditSession returns.
use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{fence, AtomicU32, Ordering},
    thread::{self, ThreadId},
};
use windows::{
    core::{IUnknown, IUnknown_Vtbl, Interface, GUID, HRESULT},
    Win32::{Foundation::*, UI::TextServices::*},
};
type Action = unsafe extern "C" fn(*mut c_void, u32) -> HRESULT;
type Destroy = unsafe extern "C" fn(*mut c_void);
#[repr(C)]
struct Session {
    vtable: *const ITfEditSession_Vtbl,
    refs: AtomicU32,
    owner: ThreadId,
    context: *mut c_void,
    action: Action,
    destroy: Destroy,
    _module: super::ModuleLifetime,
}
unsafe fn session<'a>(this: *mut c_void) -> &'a Session {
    &*this.cast::<Session>()
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
    if *iid != IUnknown::IID && *iid != ITfEditSession::IID {
        return E_NOINTERFACE;
    }
    add_ref(this);
    out.write(this);
    S_OK
}
unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    session(this).refs.fetch_add(1, Ordering::Relaxed) + 1
}
unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let count = session(this).refs.fetch_sub(1, Ordering::Release) - 1;
    if count == 0 {
        fence(Ordering::Acquire);
        let Session {
            destroy,
            context,
            _module: module,
            ..
        } = *Box::from_raw(this.cast::<Session>());
        // The destroy callback can release the final pin on the shell's code.
        destroy(context);
        drop(module);
    }
    count
}
unsafe extern "system" fn edit(this: *mut c_void, cookie: u32) -> HRESULT {
    let value = session(this);
    if value.owner != thread::current().id() {
        return RPC_E_WRONG_THREAD;
    }
    (value.action)(value.context, cookie)
}
static VTABLE: ITfEditSession_Vtbl = ITfEditSession_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: query,
        AddRef: add_ref,
        Release: release,
    },
    DoEditSession: edit,
};
/// Transfers ownership of `action_context` to the session on every path, including
/// invalid input/rejected requests. Destroy is called exactly once. The supplied
/// context interface is borrowed only for the duration of this call. An async
/// host retains its own session COM reference until execution/cancellation.
/// # Safety
/// `context` is null or a live ITfContext pointer. Both callbacks must remain
/// callable until destroy runs; invoke and release on the creating TSF thread.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_request_edit_session(
    context: *mut c_void,
    client: u32,
    flags: u32,
    action_context: *mut c_void,
    action: Option<Action>,
    destroy: Destroy,
) -> HRESULT {
    let Some(action) = action else {
        destroy(action_context);
        return E_INVALIDARG;
    };
    if context.is_null() {
        destroy(action_context);
        return E_INVALIDARG;
    }
    let value = Box::into_raw(Box::new(Session {
        vtable: &VTABLE,
        refs: AtomicU32::new(1),
        owner: thread::current().id(),
        context: action_context,
        action,
        destroy,
        _module: super::ModuleLifetime::new(),
    }));
    let session = ITfEditSession::from_raw(value.cast());
    let Some(context) = ITfContext::from_raw_borrowed(&context) else {
        return E_INVALIDARG;
    };
    let result = context.RequestEditSession(client, &session, TF_CONTEXT_EDIT_CONTEXT_FLAGS(flags));
    // Drop our initial reference; the context's queued reference remains alive.
    drop(session);
    result.unwrap_or_else(|error| error.code())
}
