// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Owned commit/selection adapters: only reply-value application remains external.
use super::*;
use crate::tsf::commit_request;
use std::rc::Rc;

type Apply = unsafe extern "C" fn(*mut c_void, u32, *mut c_void, *const u8, u32) -> i32;
type Notice = unsafe extern "C" fn(*mut c_void);
type KeyNotice = unsafe extern "C" fn(*mut c_void, u32, i32);
struct Owner {
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: CString,
    adapter: *mut c_void,
    apply: Option<Apply>,
    sink: *mut c_void,
    notice: Option<Notice>,
    key_notice: Option<KeyNotice>,
}
struct Reply<'a> {
    owner: &'a Owner,
    cookie: u32,
    context: *mut c_void,
    target: *mut c_void,
}
unsafe extern "C" fn current(
    data: *mut c_void,
    context: *mut *mut c_void,
    sink: *mut *mut c_void,
) -> *mut c_void {
    let s = (*data.cast::<Owner>()).state;
    context.write((*s).composition_context);
    sink.write((*s).edit_context);
    (*s).composition
}
unsafe extern "C" fn update(data: *mut c_void, op: u32, cookie: u32, value: i32) {
    let o = &*data.cast::<Owner>();
    match op {
        0 | 4 => (*o.state).own_edit = value != 0,
        1 => (*o.state).keep_caret = value != 0,
        2 => {
            super::owned::meltype_tsf_service_end_composition(o.state, o.lifetime, cookie);
        }
        3 => meltype_tsf_service_hide_candidates(o.state),
        _ => {}
    }
}
unsafe extern "C" fn parsed(data: *mut c_void, bytes: *const u8, length: u32) -> i32 {
    let r = &*data.cast::<Reply>();
    if !crate::json_bridge::valid(std::slice::from_raw_parts(bytes, length as usize)) {
        return 0;
    }
    let s = r.owner.state;
    if (*s).composition != r.target || (*s).composition_context != r.context {
        return 1;
    }
    match r.owner.apply {
        Some(apply) => apply(r.owner.adapter, r.cookie, r.context, bytes, length),
        None => i32::from(
            super::apply::meltype_tsf_service_apply_reply(
                s,
                r.owner.lifetime,
                r.owner.sink,
                r.owner.worker.as_ptr(),
                r.context,
                r.cookie,
                bytes,
                length,
                data,
                Some(failed),
            ) >= 0,
        ),
    }
}
unsafe extern "C" fn failed(data: *mut c_void) {
    let r = &*data.cast::<Reply>();
    if let Some(notice) = r.owner.notice {
        notice(r.owner.adapter);
    }
    if let Some(notice) = r.owner.key_notice {
        notice(r.owner.adapter, 4, 0);
    }
}
unsafe extern "C" fn request(
    data: *mut c_void,
    text: *const c_char,
    cookie: u32,
    context: *mut c_void,
    target: *mut c_void,
) {
    let o = &*data.cast::<Owner>();
    let bytes = CStr::from_ptr(text).to_bytes();
    let Ok(length) = u32::try_from(bytes.len()) else {
        return;
    };
    let mut reply = Reply {
        owner: o,
        cookie,
        context,
        target,
    };
    super::exchange::meltype_tsf_service_exchange(
        o.state,
        o.worker.as_ptr(),
        o.lifetime,
        bytes.as_ptr(),
        length,
        800,
        0,
        ptr::addr_of_mut!(reply).cast(),
        Some(parsed),
        Some(failed),
    );
}
unsafe fn owner(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    adapter: *mut c_void,
    apply: Option<Apply>,
    notice: Option<Notice>,
) -> Option<Rc<Owner>> {
    let apply = apply?;
    if state.is_null() || lifetime.is_null() || worker.is_null() {
        return None;
    }
    Some(Rc::new(Owner {
        state,
        lifetime,
        worker: CStr::from_ptr(worker).to_owned(),
        adapter,
        apply: Some(apply),
        sink: ptr::null_mut(),
        notice,
        key_notice: None,
    }))
}
fn callbacks(o: &Rc<Owner>) -> commit_request::Callbacks {
    commit_request::Callbacks {
        owner: Rc::as_ptr(o).cast_mut().cast(),
        lifetime: o.lifetime,
        current,
        update,
        request_apply: request,
    }
}
unsafe fn metadata(state: *mut ServiceState) -> CString {
    let sid = CStr::from_ptr(meltype_tsf_service_sid(state)).to_string_lossy();
    CString::new(serde_json::json!({"sid":sid}).to_string()).unwrap()
}
/// # Safety
/// Live service and root IUnknown on its TSF thread. Worker is a trusted absolute
/// UTF-8 path, copied for pending sessions. Lifetime pins state/adapter/code until
/// cancellation or execution. Apply receives borrowed valid compatible JSON and
/// must never unwind or retain arguments; it applies to the supplied edit cookie.
/// mode=0 sync,1 async,2 existing context/cookie. Notice reports invalid replies.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_commit(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    mode: u32,
    context: *mut c_void,
    cookie: u32,
    keep: i32,
    adapter: *mut c_void,
    apply: Option<Apply>,
    notice: Option<Notice>,
) -> HRESULT {
    let Some(o) = owner(state, lifetime, worker, adapter, apply, notice) else {
        return E_INVALIDARG;
    };
    let callbacks = callbacks(&o);
    let metadata = metadata(state);
    commit_request::commit(
        mode,
        context,
        (*state).client,
        cookie,
        metadata.as_ptr(),
        &callbacks,
        keep,
        Some(o),
    )
}
// Key editing can schedule a commit that outlives its stack adapter. Copy only
// the real shell adapter and notices into the independently owned commit work.
#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn from_key(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    mode: u32,
    context: *mut c_void,
    cookie: u32,
    adapter: *mut c_void,
    apply: Apply,
    notice: Option<KeyNotice>,
) -> HRESULT {
    let Some(mut o) = owner(state, lifetime, worker, adapter, Some(apply), None) else {
        return E_INVALIDARG;
    };
    Rc::get_mut(&mut o).unwrap().key_notice = notice;
    let callbacks = callbacks(&o);
    let metadata = metadata(state);
    commit_request::commit(
        mode,
        context,
        (*state).client,
        cookie,
        metadata.as_ptr(),
        &callbacks,
        1,
        Some(o),
    )
}
// Complete native commit path; no shell operation or value callback is retained.
#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn native(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    worker: *const c_char,
    mode: u32,
    context: *mut c_void,
    cookie: u32,
    keep: i32,
    adapter: *mut c_void,
    notice: Option<KeyNotice>,
) -> HRESULT {
    if state.is_null() || lifetime.is_null() || worker.is_null() {
        return E_INVALIDARG;
    }
    let o = Rc::new(Owner {
        state,
        lifetime,
        sink,
        worker: CStr::from_ptr(worker).to_owned(),
        adapter,
        apply: None,
        notice: None,
        key_notice: notice,
    });
    let callbacks = callbacks(&o);
    let metadata = metadata(state);
    commit_request::commit(
        mode,
        context,
        (*state).client,
        cookie,
        metadata.as_ptr(),
        &callbacks,
        keep,
        Some(o),
    )
}
/// # Safety
/// Same service/worker/callback contract as service_commit. Pending selection
/// retains its adapter and lifetime; context/target must still match at execution.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_select_candidate(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    worker: *const c_char,
    index: i32,
    prediction: i32,
    adapter: *mut c_void,
    apply: Option<Apply>,
    notice: Option<Notice>,
) -> HRESULT {
    let Some(o) = owner(state, lifetime, worker, adapter, apply, notice) else {
        return E_INVALIDARG;
    };
    let callbacks = callbacks(&o);
    let metadata = metadata(state);
    commit_request::select_candidate(
        (*state).client,
        index,
        prediction,
        metadata.as_ptr(),
        &callbacks,
        Some(o),
    )
}
