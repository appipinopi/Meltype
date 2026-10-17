// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Native follow-up operations for focus, edit and mode notifications.
use super::*;
use windows::core::GUID;
type Notice = unsafe extern "C" fn(*mut c_void, u32, i32);
struct Owner {
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    worker: *const c_char,
    adapter: *mut c_void,
    notice: Option<Notice>,
}
unsafe extern "C" fn relay(data: *mut c_void, op: u32, value: i32) {
    let o = &*data.cast::<Owner>();
    if let Some(notice) = o.notice {
        notice(o.adapter, op, value);
    }
}
unsafe extern "C" fn failure(data: *mut c_void) {
    relay(data, 1, 0);
}
unsafe extern "C" fn focus_commit(data: *mut c_void, mode: i32) -> i32 {
    let o = &*data.cast::<Owner>();
    let result = super::commit::notification(
        o.state,
        o.lifetime,
        o.sink,
        o.worker,
        u32::from(mode != 0),
        1,
        o.adapter,
        o.notice,
    );
    if mode == 0 {
        i32::from(result.is_ok())
    } else {
        1
    }
}
unsafe extern "C" fn resume(data: *mut c_void) {
    let o = &*data.cast::<Owner>();
    if super::recovered::meltype_tsf_service_resume(
        o.state,
        o.lifetime,
        o.worker,
        o.sink,
        ptr::null_mut(),
        data,
        Some(failure),
    ) == 1
    {
        relay(data, 7, 1);
    }
}
unsafe extern "C" fn query(data: *mut c_void, cookie: u32, context: *mut c_void, changed: i32) {
    let o = &*data.cast::<Owner>();
    super::mode_action::meltype_tsf_service_mode_action(
        o.state, o.lifetime, o.worker, context, cookie, 2, changed, o.adapter, o.notice,
    );
}
unsafe extern "C" fn edited_commit(data: *mut c_void) {
    let o = &*data.cast::<Owner>();
    relay(data, 10, 0);
    let _ = super::commit::notification(
        o.state, o.lifetime, o.sink, o.worker, 1, 1, o.adapter, o.notice,
    );
}
unsafe extern "C" fn opened_commit(data: *mut c_void) {
    let o = &*data.cast::<Owner>();
    let _ = super::commit::notification(
        o.state, o.lifetime, o.sink, o.worker, 1, 0, o.adapter, o.notice,
    );
}
unsafe extern "C" fn opened_notice(data: *mut c_void, open: i32) {
    relay(data, 11, open);
}
/// # Safety
/// Live state/root/sinks/document on its TSF thread; worker trusted absolute UTF8.
/// Root pins state and callbacks. Notice uses mode-action codes plus7=resumed,
/// 10=caret-outside commit,11=open state; op1 also reports deferred invalid replies. Callbacks never unwind; async work owns real adapter/notice.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_focus_native(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    edit_sink: *mut c_void,
    worker: *const c_char,
    document: *mut c_void,
    event: i32,
    adapter: *mut c_void,
    notice: Option<Notice>,
) -> HRESULT {
    let mut o = Owner {
        state,
        lifetime,
        sink,
        worker,
        adapter,
        notice,
    };
    super::focus::meltype_tsf_service_focus(
        state,
        worker,
        lifetime,
        edit_sink,
        document,
        event,
        ptr::addr_of_mut!(o).cast(),
        Some(focus_commit),
        Some(resume),
        Some(relay),
    )
}
/// # Safety
/// Same thread/lifetime/worker contract as focus_native; valid context/record and
/// read cookie. Record may be null. Pending commit work never retains this stack.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_edited_native(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    worker: *const c_char,
    context: *mut c_void,
    record: *mut c_void,
    cookie: u32,
    adapter: *mut c_void,
    notice: Option<Notice>,
) -> i32 {
    if worker.is_null() {
        return 0;
    }
    let mut o = Owner {
        state,
        lifetime,
        sink,
        worker,
        adapter,
        notice,
    };
    super::edited::meltype_tsf_service_edited(
        state,
        lifetime,
        context,
        record,
        cookie,
        ptr::addr_of_mut!(o).cast(),
        Some(query),
        Some(edited_commit),
    )
}
/// # Safety
/// Same thread/lifetime/worker contract; readable GUID. Commit on closing does
/// not preserve the caret, matching the original compartment-notification path.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_opened_native(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    worker: *const c_char,
    guid: *const GUID,
    adapter: *mut c_void,
    notice: Option<Notice>,
) -> HRESULT {
    if worker.is_null() {
        return E_INVALIDARG;
    }
    let mut o = Owner {
        state,
        lifetime,
        sink,
        worker,
        adapter,
        notice,
    };
    super::opened::meltype_tsf_service_opened(
        state,
        lifetime,
        guid,
        ptr::addr_of_mut!(o).cast(),
        Some(opened_commit),
        Some(opened_notice),
    )
}
