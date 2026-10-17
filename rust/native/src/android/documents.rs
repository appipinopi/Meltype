// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! ContentResolver transfer and restore staging. No lock crosses a Java callback.
//! Each confirmation identifies one owned temporary file, never a later import.
use super::*;
use jni::objects::JValueOwned;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use tempfile::TempPath;

type Result<T> = std::result::Result<T, String>;
const LIMIT: usize = 32 * 1024 * 1024;
const BUFFER: usize = 8192;
static NEXT: AtomicU64 = AtomicU64::new(1);
struct State {
    token: u64,
    pending: Option<TempPath>,
}
fn states() -> &'static Mutex<HashMap<jlong, State>> {
    static STATES: OnceLock<Mutex<HashMap<jlong, State>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}
pub(super) fn create(handle: jlong) {
    if handle != 0 {
        states().lock().unwrap_or_else(|e| e.into_inner()).insert(
            handle,
            State {
                token: NEXT.fetch_add(1, Ordering::Relaxed),
                pending: None,
            },
        );
    }
}
pub(super) fn destroy(handle: jlong) {
    let removed = states()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&handle);
    drop(removed);
}
fn begin(handle: jlong) -> Result<u64> {
    let (token, old) = {
        let mut states = states().lock().unwrap_or_else(|e| e.into_inner());
        let state = states
            .get_mut(&handle)
            .ok_or("設定画面は閉じられています")?;
        state.token = NEXT.fetch_add(1, Ordering::Relaxed);
        (state.token, state.pending.take())
    };
    drop(old);
    Ok(token)
}
fn current(handle: jlong, token: u64) -> Result<()> {
    if states()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&handle)
        .is_some_and(|s| s.token == token)
    {
        Ok(())
    } else {
        Err("ファイル操作は取り消されました".into())
    }
}
fn checked<T>(env: &mut JNIEnv, result: jni::errors::Result<T>) -> Result<T> {
    result.map_err(|error| {
        let mut message = error.to_string();
        if env.exception_check().unwrap_or(false) {
            let exception = env.exception_occurred().ok();
            let _ = env.exception_clear();
            if let Some(exception) = exception {
                if let Ok(text) = env
                    .call_method(exception, "toString", "()Ljava/lang/String;", &[])
                    .and_then(|v| v.l())
                {
                    if let Ok(text) = env.get_string(&JString::from(text)) {
                        message = text.into();
                    }
                }
            }
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
        }
        message
    })
}
fn call<'a>(
    env: &mut JNIEnv<'a>,
    object: &JObject,
    name: &str,
    signature: &str,
    args: &[JValue],
) -> Result<JValueOwned<'a>> {
    let result = env.call_method(object, name, signature, args);
    checked(env, result)
}
fn text(env: &mut JNIEnv, value: JString) -> Result<String> {
    let result = env.get_string(&value).map(String::from);
    checked(env, result)
}
fn close(env: &mut JNIEnv, stream: &JObject, result: Result<()>) -> Result<()> {
    let closed = call(env, stream, "close", "()V", &[]).map(|_| ());
    result.and(closed)
}
fn read_document(
    env: &mut JNIEnv,
    resolver: &JObject,
    uri: &JObject,
    file: &mut std::fs::File,
    handle: jlong,
    token: u64,
) -> Result<()> {
    let stream = call(
        env,
        resolver,
        "openInputStream",
        "(Landroid/net/Uri;)Ljava/io/InputStream;",
        &[JValue::Object(uri)],
    )?
    .l()
    .map_err(|e| e.to_string())?;
    if stream.is_null() {
        return Err("ファイルを開けません".into());
    }
    let result = (|| {
        let buffer = env.new_byte_array(BUFFER as i32);
        let buffer = checked(env, buffer)?;
        let mut total = 0usize;
        loop {
            current(handle, token)?;
            let count = call(env, &stream, "read", "([B)I", &[JValue::Object(&buffer)])?
                .i()
                .map_err(|e| e.to_string())?;
            current(handle, token)?;
            if count == -1 {
                break;
            }
            if count <= 0 || count as usize > BUFFER {
                return Err("読込サイズが正しくありません".into());
            }
            total += count as usize;
            if total > LIMIT {
                return Err("上限は32 MiBです".into());
            }
            let bytes = env.convert_byte_array(&buffer);
            let bytes = checked(env, bytes)?;
            file.write_all(&bytes[..count as usize])
                .map_err(|e| e.to_string())?;
        }
        file.flush().map_err(|e| e.to_string())
    })();
    close(env, &stream, result)
}
fn write_document(
    env: &mut JNIEnv,
    resolver: &JObject,
    uri: &JObject,
    file: &TempPath,
    handle: jlong,
    token: u64,
) -> Result<()> {
    current(handle, token)?;
    let mode = env.new_string("wt");
    let mode = checked(env, mode)?;
    let stream = call(
        env,
        resolver,
        "openOutputStream",
        "(Landroid/net/Uri;Ljava/lang/String;)Ljava/io/OutputStream;",
        &[JValue::Object(uri), JValue::Object(&mode)],
    )?
    .l()
    .map_err(|e| e.to_string())?;
    if stream.is_null() {
        return Err("保存先を開けません".into());
    }
    let result = (|| {
        let buffer = env.new_byte_array(BUFFER as i32);
        let buffer = checked(env, buffer)?;
        let mut input = std::fs::File::open(file).map_err(|e| e.to_string())?;
        let mut bytes = [0u8; BUFFER];
        loop {
            current(handle, token)?;
            let count = input.read(&mut bytes).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            let signed: Vec<i8> = bytes[..count].iter().map(|b| *b as i8).collect();
            let result = env.set_byte_array_region(&buffer, 0, &signed);
            checked(env, result)?;
            call(
                env,
                &stream,
                "write",
                "([BII)V",
                &[
                    JValue::Object(&buffer),
                    JValue::Int(0),
                    JValue::Int(count as i32),
                ],
            )?;
        }
        current(handle, token)
    })();
    close(env, &stream, result)
}
fn transfer(
    env: &mut JNIEnv,
    handle: jlong,
    context: JObject,
    uri: JObject,
    operation: JString,
) -> Result<Value> {
    let operation = text(env, operation)?;
    let output = match operation.as_str() {
        "dictionary_export" | "backup_create" => true,
        "dictionary_import" | "backup_restore" => false,
        _ => return Err("ファイル操作が見つかりません".into()),
    };
    let token = begin(handle)?;
    let cache = call(env, &context, "getCacheDir", "()Ljava/io/File;", &[])?
        .l()
        .map_err(|e| e.to_string())?;
    let path = call(env, &cache, "getAbsolutePath", "()Ljava/lang/String;", &[])?
        .l()
        .map_err(|e| e.to_string())?;
    let path = text(env, JString::from(path))?;
    current(handle, token)?;
    let file = tempfile::Builder::new()
        .prefix("meltype-document-")
        .tempfile_in(path)
        .map_err(|e| e.to_string())?
        .into_temp_path();
    let path = file
        .to_str()
        .ok_or("ファイル名が正しくありません")?
        .to_owned();
    let resolver = call(
        env,
        &context,
        "getContentResolver",
        "()Landroid/content/ContentResolver;",
        &[],
    )?
    .l()
    .map_err(|e| e.to_string())?;
    current(handle, token)?;
    if !output {
        let mut input = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&file)
            .map_err(|e| e.to_string())?;
        read_document(env, &resolver, &uri, &mut input, handle, token)?;
    }
    current(handle, token)?;
    if operation == "backup_restore" {
        let report = settings::request(handle, json!({"op":"backup_inspect","path":path}))?;
        let message = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        let mut states = states().lock().unwrap_or_else(|e| e.into_inner());
        let state = states
            .get_mut(&handle)
            .filter(|s| s.token == token)
            .ok_or("ファイル操作は取り消されました")?;
        state.pending = Some(file);
        return Ok(
            json!({"confirm":{"title":"バックアップを復元","message":message,"token":token}}),
        );
    }
    settings::request(handle, json!({"op":operation,"path":path}))?;
    if output {
        write_document(env, &resolver, &uri, &file, handle, token)?;
    }
    current(handle, token)?;
    Ok(if operation == "dictionary_import" {
        json!({"page":1})
    } else {
        json!({})
    })
}
fn restore(handle: jlong, token: u64, accept: bool) -> Result<Value> {
    let file = {
        let mut states = states().lock().unwrap_or_else(|e| e.into_inner());
        let state = states.get_mut(&handle).filter(|s| s.token == token);
        match state {
            Some(state) => state.pending.take(),
            None if !accept => return Ok(json!({})),
            None => return Err("復元対象は取り消されました".into()),
        }
    };
    if !accept {
        return Ok(json!({}));
    }
    let file = file.ok_or("復元対象がありません")?;
    let path = file.to_str().ok_or("ファイル名が正しくありません")?;
    current(handle, token)?;
    settings::request(handle, json!({"op":"backup_restore","path":path}))?;
    Ok(json!({}))
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_settingsDocument(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    context: JObject,
    uri: JObject,
    operation: JString,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        transfer(&mut env, handle, context, uri, operation)
    }))
    .unwrap_or_else(|_| Err("ファイル操作に失敗しました".into()));
    settings::reply(&mut env, result)
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_settingsRestore(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    token: jlong,
    accept: jboolean,
) -> jstring {
    let result = std::panic::catch_unwind(|| restore(handle, token as u64, accept != 0))
        .unwrap_or_else(|_| Err("復元に失敗しました".into()));
    settings::reply(&mut env, result)
}
