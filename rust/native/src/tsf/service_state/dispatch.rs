// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Complete key entry dispatch with native commit/edit/reply operations.
use super::*;
use windows::Win32::UI::Input::KeyboardAndMouse::INPUT;
type Notice = unsafe extern "C" fn(*mut c_void, u32, i32);
type Sender = unsafe extern "C" fn(*mut c_void, *const INPUT, u32) -> u32;
struct Owner {
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    worker: *const c_char,
    adapter: *mut c_void,
    mode_notice: Option<Notice>,
    key_notice: Option<Notice>,
    sender: Option<Sender>,
}
unsafe extern "C" fn commit(data: *mut c_void, mode: u32, keep: i32) -> i32 {
    let o = &*data.cast::<Owner>();
    let status = super::commit::native(
        o.state,
        o.lifetime,
        o.sink,
        o.worker,
        mode,
        ptr::null_mut(),
        0,
        keep,
        o.adapter,
        o.key_notice,
    );
    if mode == 0 {
        i32::from(status.is_ok())
    } else {
        1
    }
}
unsafe extern "C" fn handle(data: *mut c_void, context: *mut c_void, key: u32, ch: u16) {
    let o = &*data.cast::<Owner>();
    let _ = super::key_work::run(
        o.state,
        o.lifetime,
        o.sink,
        o.worker,
        context,
        key,
        ch,
        o.adapter,
        None,
        o.key_notice,
        o.sender,
    );
}
unsafe extern "C" fn notice(data: *mut c_void, op: u32, value: i32) {
    let o = &*data.cast::<Owner>();
    if let Some(notice) = o.mode_notice {
        notice(o.adapter, op, value);
    }
}
/// # Safety
/// Live state/root/context/composition sink on its TSF thread. Worker is trusted
/// absolute UTF-8. Root pins state/notice/sender code through reentry. Notices
/// follow service key-event and handle-key codes respectively; callbacks never
/// unwind. Optional sender replaces real SendInput. Native async commits retain
/// their own worker/adapter/notice and never retain this stack. kind0..3 matches
/// test-down/down/test-up/up. Returns whether the key was eaten.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_dispatch_key(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    worker: *const c_char,
    kind: u32,
    context: *mut c_void,
    key: u32,
    message: u64,
    adapter: *mut c_void,
    mode_notice: Option<Notice>,
    key_notice: Option<Notice>,
    sender: Option<Sender>,
) -> i32 {
    if state.is_null() || lifetime.is_null() || worker.is_null() {
        return 0;
    }
    let mut owner = Owner {
        state,
        lifetime,
        sink,
        worker,
        adapter,
        mode_notice,
        key_notice,
        sender,
    };
    super::key_input::meltype_tsf_service_key_event(
        state,
        lifetime,
        sink,
        worker,
        kind,
        context,
        key,
        message,
        ptr::addr_of_mut!(owner).cast(),
        Some(commit),
        Some(handle),
        Some(notice),
    )
}
