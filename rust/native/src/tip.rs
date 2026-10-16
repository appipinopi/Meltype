// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! In-process adapter for upstream's TSF JSON protocol. Each frontend owns a
//! connection; session IDs cannot address sessions in other connections.
use super::*;
use serde_json::{json, Value};
static CONNECTIONS: OnceLock<Mutex<HashMap<usize, HashMap<String, usize>>>> = OnceLock::new();
fn connections() -> &'static Mutex<HashMap<usize, HashMap<String, usize>>> {
    CONNECTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[no_mangle]
pub extern "C" fn meltype_tip_create() -> *mut c_void {
    let Ok(mut all) = connections().lock() else {
        return std::ptr::null_mut();
    };
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    all.insert(id, HashMap::new());
    id as *mut c_void
}
#[no_mangle]
pub extern "C" fn meltype_tip_destroy(handle: *mut c_void) {
    if let Ok(mut all) = connections().lock() {
        if let Some(sessions) = all.remove(&(handle as usize)) {
            for (_, id) in sessions {
                meltype_destroy(id as *mut c_void);
            }
        }
    }
}
/// # Safety
/// request must be a valid NUL-terminated UTF-8 JSON string of at most 64 KiB.
#[no_mangle]
pub unsafe extern "C" fn meltype_tip_request(
    connection: *mut c_void,
    request: *const c_char,
) -> *mut c_char {
    catch_unwind(AssertUnwindSafe(|| request_inner(connection, request)))
        .ok()
        .flatten()
        .map(|s| allocate(&s))
        .unwrap_or(std::ptr::null_mut())
}
unsafe fn request_inner(connection: *mut c_void, request: *const c_char) -> Option<String> {
    let text = read(request)?;
    if text.len() > 65536 {
        return None;
    }
    let request: Value = serde_json::from_str(&text).ok()?;
    let mut all = connections().lock().ok()?;
    let sessions = all.get_mut(&(connection as usize))?;
    let inactive = || json!({"active":false,"consumed":false,"commits":[],"view":null}).to_string();
    let op = request["op"].as_str()?;
    if op == "hello" {
        return Some(json!({"active":true,"code":false,"directDetection":false,"consumed":false,"commits":[],"view":null}).to_string());
    }
    let sid = request["sid"].as_str()?;
    if sid.len() > 256 {
        return None;
    }
    if op == "close" {
        if let Some(id) = sessions.remove(sid) {
            meltype_destroy(id as *mut c_void);
        }
        return Some(inactive());
    }
    if !sessions.contains_key(sid) {
        if request["composing"] == true
            || sessions.len() >= 16
            || !["key", "mode", "toggle"].contains(&op)
        {
            return Some(inactive());
        }
        let handle = meltype_create();
        if handle.is_null() {
            return None;
        }
        sessions.insert(sid.to_owned(), handle as usize);
    }
    let handle = *sessions.get(sid)? as *mut c_void;
    let number = |name: &str| {
        request[name]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(0)
    };
    let result = match op {
        "key" => {
            meltype_set_direct(handle, i32::from(request["direct"] == true));
            meltype_set_private(handle, i32::from(request["private"] == true));
            let before = request["before"]
                .as_str()
                .and_then(|s| CString::new(s).ok());
            meltype_handle_key(
                handle,
                number("vk"),
                number("ch"),
                number("mods"),
                before.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                std::ptr::null(),
            )
        }
        "commit" => meltype_commit(handle),
        "select" if request["prediction"] != true => {
            meltype_select_candidate(handle, number("index"))
        }
        "mode" | "toggle" => run(handle, |s| s.result(false)),
        _ => return Some(inactive()),
    };
    let text = take(result)?;
    let mut value: Value = serde_json::from_str(&text).ok()?;
    value["active"] = json!(true);
    value["code"] = json!(false);
    value["english"] = json!(request["direct"] == true);
    value["directDetection"] = json!(false);
    Some(value.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sessions_are_scoped_to_connection() {
        unsafe {
            let a = meltype_tip_create();
            let b = meltype_tip_create();
            let key = CString::new(r#"{"op":"key","sid":"same","ch":107}"#).unwrap();
            let result = meltype_tip_request(a, key.as_ptr());
            assert!(!result.is_null());
            meltype_free(result.cast());
            let commit = CString::new(r#"{"op":"commit","sid":"same"}"#).unwrap();
            let value: Value =
                serde_json::from_str(&take(meltype_tip_request(b, commit.as_ptr())).unwrap())
                    .unwrap();
            assert_eq!(value["active"], false);
            meltype_tip_destroy(a);
            meltype_tip_destroy(b);
        }
    }
}
