// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Reply projection and ordered document/UI application without shell values.
use super::*;
use crate::tsf::apply_reply;
type Notice = unsafe extern "C" fn(*mut c_void);
struct Owner {
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    worker: *const c_char,
    context: *mut c_void,
    cookie: u32,
    adapter: *mut c_void,
    notice: Option<Notice>,
}
unsafe extern "C" fn current(data: *mut c_void) -> *mut c_void {
    (*(*data.cast::<Owner>()).state).composition
}
unsafe extern "C" fn write(
    data: *mut c_void,
    range: *mut c_void,
    text: *const u16,
    length: u32,
) -> i32 {
    let o = &*data.cast::<Owner>();
    let result = super::owned::meltype_tsf_service_write_commit(
        o.state, o.lifetime, o.sink, o.context, range, o.cookie, text, length,
    );
    i32::from(result >= 0 && result & 1 != 0)
}
unsafe extern "C" fn valid(_: *mut c_void, bytes: *const u8, length: u32) -> i32 {
    i32::from(crate::json_bridge::valid(std::slice::from_raw_parts(
        bytes,
        length as usize,
    )))
}
unsafe extern "C" fn failed(data: *mut c_void) {
    let o = &*data.cast::<Owner>();
    if let Some(notice) = o.notice {
        notice(o.adapter);
    }
}
unsafe extern "C" fn action(
    data: *mut c_void,
    op: u32,
    cookie: u32,
    context: *mut c_void,
    json: *const c_char,
    value: i32,
) {
    let o = &*data.cast::<Owner>();
    let s = o.state;
    match op {
        0 => (*s).last_converting = value != 0,
        1 => {
            super::owned::meltype_tsf_service_apply_composition(
                s, o.lifetime, o.sink, context, cookie, json,
            );
        }
        2 => {
            meltype_tsf_service_show_candidates(s, context, (*s).composition, cookie, json);
        }
        3 => {
            super::owned::meltype_tsf_service_end_composition(s, o.lifetime, cookie);
        }
        4 => meltype_tsf_service_hide_candidates(s),
        5 => {
            let bytes = CStr::from_ptr(json).to_bytes();
            if let Ok(length) = u32::try_from(bytes.len()) {
                super::exchange::meltype_tsf_service_exchange(
                    s,
                    o.worker,
                    o.lifetime,
                    bytes.as_ptr(),
                    length,
                    800,
                    0,
                    data,
                    Some(valid),
                    Some(failed),
                );
            }
            (*s).key_failed = true;
            (*s).consumed = false;
        }
        _ => {}
    }
}
/// # Safety
/// Live service/root/context/composition sink on its TSF thread, read/write cookie.
/// Worker is trusted absolute UTF-8; reply is readable for length bytes. Root pins
/// state/notice code throughout reentrant COM/UI. Notice never unwinds. Returns
/// -1 invalid arguments/JSON,0 failed guarded application,1 applied. Projection
/// matches the old shell's typed values, lossy UTF-16 and numeric conversion.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_service_apply_reply(
    state: *mut ServiceState,
    lifetime: *mut c_void,
    sink: *mut c_void,
    worker: *const c_char,
    context: *mut c_void,
    cookie: u32,
    reply: *const u8,
    length: u32,
    adapter: *mut c_void,
    notice: Option<Notice>,
) -> i32 {
    if state.is_null()
        || lifetime.is_null()
        || worker.is_null()
        || context.is_null()
        || reply.is_null()
    {
        return -1;
    }
    let Some(reply) =
        crate::json_bridge::application_reply(std::slice::from_raw_parts(reply, length as usize))
    else {
        return -1;
    };
    let sid = CStr::from_ptr(meltype_tsf_service_sid(state)).to_string_lossy();
    let metadata =
        CString::new(serde_json::json!({"sid":sid,"direct":!(*state).open}).to_string()).unwrap();
    let reply = CString::new(reply.to_string()).unwrap();
    let mut owner = Owner {
        state,
        lifetime,
        sink,
        worker,
        context,
        cookie,
        adapter,
        notice,
    };
    let callbacks = apply_reply::Callbacks {
        owner: ptr::addr_of_mut!(owner).cast(),
        lifetime,
        current,
        write,
        action,
    };
    apply_reply::meltype_tsf_apply_reply(
        context,
        cookie,
        reply.as_ptr(),
        metadata.as_ptr(),
        &callbacks,
    )
}
