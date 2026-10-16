// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use super::*;
use jni::{
    objects::{JClass, JString},
    sys::{jboolean, jint, jlong, jstring},
    JNIEnv,
};
fn java_result(env: &mut JNIEnv, result: *mut c_char) -> jstring {
    let text = unsafe { take(result) }
        .unwrap_or_else(|| "{\"consumed\":false,\"commits\":[],\"view\":null}".into());
    env.new_string(text)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_create(
    mut env: JNIEnv,
    _class: JClass,
    directory: JString,
) -> jlong {
    let Ok(directory) = env.get_string(&directory) else {
        return 0;
    };
    let Ok(directory) = CString::new(String::from(directory)) else {
        return 0;
    };
    unsafe { meltype_create_at(directory.as_ptr()) as jlong }
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_control(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    request: JString,
) -> jstring {
    let Ok(request) = env.get_string(&request) else {
        return std::ptr::null_mut();
    };
    let Ok(request) = CString::new(String::from(request)) else {
        return std::ptr::null_mut();
    };
    let result = unsafe { meltype_control(handle as *mut c_void, request.as_ptr()) };
    java_result(&mut env, result)
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_predict(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    index: jint,
) -> jstring {
    java_result(
        &mut env,
        meltype_select_prediction(handle as *mut c_void, index),
    )
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_destroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    meltype_destroy(handle as *mut c_void);
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_key(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    vk: jint,
    ch: jint,
    modifiers: jint,
) -> jstring {
    let result = unsafe {
        meltype_handle_key(
            handle as *mut c_void,
            vk,
            ch,
            modifiers,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    java_result(&mut env, result)
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_commit(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    java_result(&mut env, meltype_commit(handle as *mut c_void))
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_reset(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    java_result(&mut env, meltype_reset(handle as *mut c_void))
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_select(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    index: jint,
) -> jstring {
    java_result(
        &mut env,
        meltype_select_candidate(handle as *mut c_void, index),
    )
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_direct(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    enabled: jboolean,
) {
    meltype_set_direct(handle as *mut c_void, enabled as i32);
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_privateInput(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    enabled: jboolean,
) {
    meltype_set_private(handle as *mut c_void, enabled as i32);
}
