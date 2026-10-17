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
    picker: Option<(u64, String)>,
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
                picker: None,
            },
        );
    }
}
pub(super) fn specification(operation: &str) -> Result<(bool, &'static str)> {
    match operation {
        "dictionary_import" | "backup_restore" => Ok((false, "")),
        "dictionary_export" => Ok((true, "meltype-dictionary.txt")),
        "backup_create" => Ok((true, "meltype-backup.json")),
        _ => Err("ファイル操作が見つかりません".into()),
    }
}
fn picker(handle: jlong) -> Option<String> {
    states()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&handle)
        .and_then(|s| s.picker.as_ref().map(|(_, operation)| operation.clone()))
}
fn set_picker(handle: jlong, operation: Option<String>) -> Result<u64> {
    let mut states = states().lock().unwrap_or_else(|e| e.into_inner());
    let state = states
        .get_mut(&handle)
        .ok_or("設定画面は閉じられています")?;
    let token = NEXT.fetch_add(1, Ordering::Relaxed);
    state.picker = operation.map(|operation| (token, operation));
    Ok(token)
}
fn cancel_picker(handle: jlong, token: u64) {
    if let Some(state) = states()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&handle)
    {
        if state
            .picker
            .as_ref()
            .is_some_and(|(current, _)| *current == token)
        {
            state.picker = None;
        }
    }
}
fn pick(env: &mut JNIEnv, handle: jlong, activity: JObject, operation: JString) -> Result<Value> {
    let operation = text(env, operation)?;
    let (output, title) = specification(&operation)?;
    // One activity-result request is outstanding at a time. In particular, an
    // old result cannot be interpreted using a later operation's direction.
    if picker(handle).is_some() {
        return Err("ファイル選択が完了していません".into());
    }
    let token = set_picker(handle, Some(operation))?;
    let result = (|| {
        let action = env.new_string(if output {
            "android.intent.action.CREATE_DOCUMENT"
        } else {
            "android.intent.action.OPEN_DOCUMENT"
        });
        let action = checked(env, action)?;
        let intent = env.new_object(
            "android/content/Intent",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&action)],
        );
        let intent = checked(env, intent)?;
        for (method, value) in [
            ("setType", "*/*"),
            ("addCategory", "android.intent.category.OPENABLE"),
        ] {
            let value = env.new_string(value);
            let value = checked(env, value)?;
            call(
                env,
                &intent,
                method,
                "(Ljava/lang/String;)Landroid/content/Intent;",
                &[JValue::Object(&value)],
            )?;
        }
        if output {
            let key = env.new_string("android.intent.extra.TITLE");
            let key = checked(env, key)?;
            let title = env.new_string(title);
            let title = checked(env, title)?;
            call(
                env,
                &intent,
                "putExtra",
                "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
                &[JValue::Object(&key), JValue::Object(&title)],
            )?;
        }
        // JNI setters/launch can reenter the activity. Do not launch for an
        // activity that was destroyed or superseded while constructing Intent.
        let expected = states()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&handle)
            .and_then(|s| s.picker.as_ref().map(|(token, _)| *token));
        if expected != Some(token) {
            return Err("ファイル選択は取り消されました".into());
        }
        call(
            env,
            &activity,
            "startActivityForResult",
            "(Landroid/content/Intent;I)V",
            &[JValue::Object(&intent), JValue::Int(1)],
        )?;
        Ok(json!({}))
    })();
    if result.is_err() {
        cancel_picker(handle, token);
    }
    result
}
fn picker_result(
    env: &mut JNIEnv,
    handle: jlong,
    context: JObject,
    code: jint,
    result: jint,
    data: JObject,
) -> Result<Value> {
    if code != 1 {
        return Ok(json!({}));
    }
    let operation = states()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&handle)
        .and_then(|s| s.picker.take())
        .map(|(_, op)| op);
    let Some(operation) = operation else {
        return Ok(json!({}));
    };
    if result != -1 || data.is_null() {
        return Ok(json!({}));
    }
    let uri = call(env, &data, "getData", "()Landroid/net/Uri;", &[])?
        .l()
        .map_err(|e| e.to_string())?;
    if uri.is_null() {
        return Ok(json!({}));
    }
    let operation = env.new_string(operation);
    let operation = checked(env, operation)?;
    transfer(env, handle, context, uri, operation)
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
    let (output, _) = specification(&operation)?;
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
pub(super) fn restore(handle: jlong, token: u64, accept: bool) -> Result<Value> {
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
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_settingsPick(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    activity: JObject,
    operation: JString,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pick(&mut env, handle, activity, operation)
    }))
    .unwrap_or_else(|_| Err("ファイル選択に失敗しました".into()));
    settings::reply(&mut env, result)
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_settingsPickerResult(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    context: JObject,
    code: jint,
    result: jint,
    data: JObject,
) -> jstring {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        picker_result(&mut env, handle, context, code, result, data)
    }))
    .unwrap_or_else(|_| Err("ファイル選択に失敗しました".into()));
    settings::reply(&mut env, result)
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_pendingDocument(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    picker(handle)
        .and_then(|value| env.new_string(value).ok())
        .map_or(std::ptr::null_mut(), JString::into_raw)
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_restoreDocumentPicker(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    operation: JString,
) {
    let value = if operation.is_null() {
        None
    } else {
        text(&mut env, operation)
            .ok()
            .filter(|s| specification(s).is_ok())
    };
    let _ = set_picker(handle, value);
}
