// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! C ABI for the existing macOS/Linux frontends and native embedding.
//! Sessions use opaque integer tokens. Stale handles fail without dereferencing
//! freed memory. Returned strings use malloc/free, including Swift callbacks.
use meltype_core::{session::Clause, Session};
mod fcitx;
mod fcitx_view;
pub mod json_bridge;
pub mod tip;
#[cfg(windows)]
pub mod tip_transport;
#[cfg(windows)]
mod tsf;
use std::{
    collections::HashMap,
    ffi::{c_char, c_void, CStr, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, OnceLock,
    },
};

pub type ClausesCallback = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_char;
pub type CandidatesCallback = unsafe extern "C" fn(*const c_char) -> *mut c_char;
pub type WordCallback = unsafe extern "C" fn(*const c_char) -> i32;

#[derive(Clone, Copy, Default)]
struct Callbacks {
    clauses: Option<ClausesCallback>,
    candidates: Option<CandidatesCallback>,
}
static CALLBACKS: Mutex<Callbacks> = Mutex::new(Callbacks {
    clauses: None,
    candidates: None,
});
static NEXT: AtomicUsize = AtomicUsize::new(1);
static MOZC: Mutex<Option<meltype_core::mozc::Mozc>> = Mutex::new(None);
fn sessions() -> &'static Mutex<HashMap<usize, Session>> {
    static SESSIONS: OnceLock<Mutex<HashMap<usize, Session>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn run(
    handle: *mut c_void,
    f: impl FnOnce(&mut Session) -> meltype_core::SessionResult,
) -> *mut c_char {
    catch_unwind(AssertUnwindSafe(|| {
        let mut all = sessions().lock().ok()?;
        let session = all.get_mut(&(handle as usize))?;
        if session.raw().is_empty() {
            if let Err(error) = session.reload() {
                session.storage_error = Some(error.to_string());
            }
        }
        Some(allocate(&serde_json::to_string(&f(session)).ok()?))
    }))
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}
fn allocate(text: &str) -> *mut c_char {
    // CString rejects interior NUL; result JSON escapes it.
    let Ok(text) = CString::new(text) else {
        return std::ptr::null_mut();
    };
    let bytes = text.as_bytes_with_nul();
    unsafe {
        let pointer = libc::malloc(bytes.len()).cast::<c_char>();
        if !pointer.is_null() {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast(), bytes.len());
        }
        pointer
    }
}
unsafe fn read(pointer: *const c_char) -> Option<String> {
    if pointer.is_null() {
        None
    } else {
        CStr::from_ptr(pointer).to_str().ok().map(str::to_owned)
    }
}
unsafe fn take(pointer: *mut c_char) -> Option<String> {
    let value = read(pointer);
    libc::free(pointer.cast());
    value
}

#[no_mangle]
pub extern "C" fn meltype_init(
    clauses: Option<ClausesCallback>,
    candidates: Option<CandidatesCallback>,
    _is_word: Option<WordCallback>,
) -> i32 {
    let Ok(mut callbacks) = CALLBACKS.lock() else {
        return 0;
    };
    *callbacks = Callbacks {
        clauses,
        candidates,
    };
    1
}

/// # Safety
/// Non-null helper/profile arguments must be valid NUL-terminated UTF-8 strings.
#[no_mangle]
pub unsafe extern "C" fn meltype_init_mozc(helper: *const c_char, profile: *const c_char) -> i32 {
    catch_unwind(|| {
        let Some(helper) = read(helper) else { return 0 };
        let profile = read(profile);
        let Ok(backend) = meltype_core::mozc::Mozc::start(
            std::path::Path::new(&helper),
            profile.as_deref().map(std::path::Path::new),
        ) else {
            return 0;
        };
        let Ok(mut mozc) = MOZC.lock() else { return 0 };
        *mozc = Some(backend);
        1
    })
    .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn meltype_create() -> *mut c_void {
    catch_unwind(|| {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        if id == 0 {
            return std::ptr::null_mut();
        }
        let Ok(mut all) = sessions().lock() else {
            return std::ptr::null_mut();
        };
        #[cfg(not(test))]
        let session = Session::open(meltype_core::storage::Store::new(
            meltype_core::storage::Store::default_directory(),
        ))
        .unwrap_or_else(|error| {
            let mut session = Session::default();
            session.storage_error = Some(error.to_string());
            session
        });
        #[cfg(test)]
        let session = Session::default();
        all.insert(id, session);
        id as *mut c_void
    })
    .unwrap_or(std::ptr::null_mut())
}
/// # Safety
/// directory must be a valid NUL-terminated UTF-8 filesystem path.
#[no_mangle]
pub unsafe extern "C" fn meltype_create_at(directory: *const c_char) -> *mut c_void {
    catch_unwind(|| {
        let Some(directory) = read(directory) else {
            return std::ptr::null_mut();
        };
        let Ok(session) = Session::open(meltype_core::storage::Store::new(directory)) else {
            return std::ptr::null_mut();
        };
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let Ok(mut all) = sessions().lock() else {
            return std::ptr::null_mut();
        };
        all.insert(id, session);
        id as *mut c_void
    })
    .unwrap_or(std::ptr::null_mut())
}
/// # Safety
/// request must be valid NUL-terminated UTF-8 JSON. Returned JSON is freed with
/// meltype_free, including error results. File operations are explicitly named.
#[no_mangle]
pub unsafe extern "C" fn meltype_control(
    handle: *mut c_void,
    request: *const c_char,
) -> *mut c_char {
    catch_unwind(AssertUnwindSafe(|| {
        let result = (|| -> Result<serde_json::Value, String> {
            let text = read(request).ok_or("invalid request")?;
            if text.len() > 1024 * 1024 {
                return Err("request too large".into());
            }
            let request = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            let mut all = sessions().lock().map_err(|e| e.to_string())?;
            all.get_mut(&(handle as usize))
                .ok_or("invalid session")?
                .control(&request)
                .map_err(|e| e.to_string())
        })();
        allocate(&match result {
            Ok(value) => value.to_string(),
            Err(error) => serde_json::json!({"error":error}).to_string(),
        })
    }))
    .unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn meltype_destroy(handle: *mut c_void) {
    fcitx::forget(handle);
    if let Ok(mut all) = sessions().lock() {
        all.remove(&(handle as usize));
    }
}

unsafe fn external_conversion(reading: &str, before: *const c_char) -> (Vec<Clause>, Vec<String>) {
    let mut external = Vec::new();
    let mut segments = Vec::new();
    if let Ok(mut mozc) = MOZC.lock() {
        if let Some(backend) = mozc.as_mut() {
            if let Ok(values) = backend.clauses(reading, &read(before).unwrap_or_default()) {
                segments = values;
            } else {
                *mozc = None;
            }
        }
    }
    let callbacks = CALLBACKS.lock().map(|c| *c).unwrap_or_default();
    let input = CString::new(reading).unwrap();
    if let Some(callback) = callbacks.clauses {
        if let Some(text) = take(callback(input.as_ptr(), before)) {
            let clauses: Option<Vec<_>> = text.lines().map(|line| line.split_once('\t')).collect();
            if let Some(clauses) = clauses {
                if clauses.iter().map(|(r, _)| *r).collect::<String>() == reading {
                    segments = clauses
                        .into_iter()
                        .map(|(r, t)| {
                            let mut candidates = vec![t.to_owned()];
                            if let (Some(callback), Ok(input)) =
                                (callbacks.candidates, CString::new(r))
                            {
                                if let Some(text) = take(callback(input.as_ptr())) {
                                    candidates.extend(
                                        text.lines().filter(|s| !s.is_empty()).map(str::to_owned),
                                    );
                                }
                            }
                            Clause::new(r.to_owned(), candidates)
                        })
                        .collect();
                }
            }
        }
    }
    if let Some(callback) = callbacks.candidates {
        if let Some(text) = take(callback(input.as_ptr())) {
            external.extend(text.lines().filter(|s| !s.is_empty()).map(str::to_owned));
        }
    }
    (segments, external)
}

/// # Safety
/// before/after, when non-null, must point to valid NUL-terminated UTF-8.
/// Calls for one session must be serialized. Registered callbacks must remain
/// valid until all sessions are destroyed, and return malloc-allocated strings.
#[no_mangle]
pub unsafe extern "C" fn meltype_handle_key(
    handle: *mut c_void,
    vk: i32,
    ch: i32,
    modifiers: i32,
    before: *const c_char,
    after: *const c_char,
) -> *mut c_char {
    catch_unwind(AssertUnwindSafe(|| {
        let reading = sessions().lock().ok().and_then(|all| {
            all.get(&(handle as usize))
                .filter(|s| {
                    vk == 0x20
                        && !s.is_converting()
                        && !s.raw().is_empty()
                        && !s.direct
                        && !s.private
                        && s.settings.boolean("Enabled")
                        && !s.code_english()
                        && (s.display(true) != s.raw() || s.settings.boolean("SeparateKeyRoles"))
                        && modifiers & 15 == 0
                })
                .map(Session::reading)
        });
        let (segments, external) = reading
            .as_deref()
            .map(|r| external_conversion(r, before))
            .unwrap_or_default();
        let result = run(handle, |s| {
            s.set_context(read(before).as_deref(), read(after).as_deref());
            if !segments.is_empty() && s.begin_clauses(segments) {
                s.result(true)
            } else if !external.is_empty() {
                s.begin_conversion(external);
                s.result(true)
            } else {
                s.key(vk, ch, modifiers)
            }
        });
        let live = if modifiers & 14 == 0 && (ch >= 0x20 || vk == 8) {
            sessions()
                .lock()
                .ok()
                .and_then(|all| all.get(&(handle as usize)).and_then(Session::live_reading))
        } else {
            None
        };
        if let Some(reading) = live {
            let (mut clauses, candidates) = external_conversion(&reading, before);
            if clauses.is_empty() && !candidates.is_empty() {
                clauses.push(Clause::new(reading, candidates));
            }
            if !clauses.is_empty() {
                if let Some(text) = take(result) {
                    if let Ok(previous) = serde_json::from_str::<meltype_core::SessionResult>(&text)
                    {
                        return run(handle, |s| {
                            s.update_live_conversion(clauses);
                            let mut result = s.result(previous.consumed);
                            result.commits = previous.commits;
                            result
                        });
                    }
                    return allocate(&text);
                }
                return std::ptr::null_mut();
            }
        }
        result
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn meltype_commit(handle: *mut c_void) -> *mut c_char {
    run(handle, Session::commit)
}
#[no_mangle]
pub extern "C" fn meltype_commit_preedit(handle: *mut c_void) -> *mut c_char {
    run(handle, Session::commit_preedit)
}
/// # Safety
/// Strings must be valid NUL-terminated UTF-8; reading may be null to infer it.
/// The host must verify selection_expect before replacing the selected text.
#[no_mangle]
pub unsafe extern "C" fn meltype_reconvert_selection(
    handle: *mut c_void,
    text: *const c_char,
    reading: *const c_char,
) -> *mut c_char {
    let Some(text) = read(text) else {
        return std::ptr::null_mut();
    };
    run(handle, |s| {
        s.reconvert_selection(&text, read(reading).as_deref())
    })
}
/// # Safety
/// before must be a valid NUL-terminated UTF-8 snapshot of text before the caret.
#[no_mangle]
pub unsafe extern "C" fn meltype_undo_commit(
    handle: *mut c_void,
    before: *const c_char,
) -> *mut c_char {
    let Some(before) = read(before) else {
        return std::ptr::null_mut();
    };
    run(handle, |s| {
        s.set_context(Some(&before), None);
        s.undo_commit()
    })
}
#[no_mangle]
pub extern "C" fn meltype_reset(handle: *mut c_void) -> *mut c_char {
    run(handle, |s| {
        s.reset();
        s.result(true)
    })
}
#[no_mangle]
pub extern "C" fn meltype_select_candidate(handle: *mut c_void, index: i32) -> *mut c_char {
    run(handle, |s| s.select(index as usize))
}
#[no_mangle]
pub extern "C" fn meltype_select_prediction(handle: *mut c_void, index: i32) -> *mut c_char {
    run(handle, |s| s.select_prediction(index as usize))
}
fn set(handle: *mut c_void, f: impl FnOnce(&mut Session)) {
    if let Ok(mut all) = sessions().lock() {
        if let Some(s) = all.get_mut(&(handle as usize)) {
            f(s);
        }
    }
}
#[no_mangle]
pub extern "C" fn meltype_set_direct(handle: *mut c_void, enabled: i32) {
    set(handle, |s| s.direct = enabled != 0);
}
#[no_mangle]
pub extern "C" fn meltype_set_direct_detection(handle: *mut c_void, enabled: i32) {
    set(handle, |s| s.direct_detection = enabled != 0);
}
#[no_mangle]
pub extern "C" fn meltype_set_code_input(handle: *mut c_void, enabled: i32) {
    set(handle, |s| s.code_input = enabled != 0);
}
#[no_mangle]
pub extern "C" fn meltype_set_private(handle: *mut c_void, enabled: i32) {
    set(handle, |s| {
        if s.private != (enabled != 0) {
            s.reset();
        }
        s.private = enabled != 0;
    });
}
// Deletion is permitted only when the host supports expected-text validation.
#[no_mangle]
pub extern "C" fn meltype_set_can_delete(handle: *mut c_void, enabled: i32) {
    set(handle, |s| s.can_delete = enabled != 0);
}
// NULL tells existing frontends to apply their regular mode-switch behavior.
#[no_mangle]
pub extern "C" fn meltype_role_key(handle: *mut c_void, vk: i32) -> *mut c_char {
    let enabled = sessions()
        .lock()
        .ok()
        .and_then(|all| {
            all.get(&(handle as usize))
                .map(|s| s.settings.boolean("SeparateKeyRoles") && !s.raw().is_empty())
        })
        .unwrap_or(false);
    if enabled {
        run(handle, |s| s.key(vk, 0, 0))
    } else {
        std::ptr::null_mut()
    }
}
#[no_mangle]
pub extern "C" fn meltype_version() -> *mut c_char {
    allocate(concat!(env!("CARGO_PKG_VERSION"), "-rust-preview"))
}
#[no_mangle]
pub extern "C" fn meltype_data_directory() -> *mut c_char {
    let base = meltype_core::storage::Store::default_directory();
    allocate(&base.to_string_lossy())
}
#[no_mangle]
pub extern "C" fn meltype_report_url(_platform: *const c_char) -> *mut c_char {
    allocate("https://github.com/appipinopi/Meltype/issues/new")
}

/// # Safety
/// pointer must be NULL or a live result from this library, freed exactly once.
#[no_mangle]
pub unsafe extern "C" fn meltype_free(pointer: *mut c_void) {
    libc::free(pointer);
}

#[cfg(any(target_os = "android", feature = "android-jni-test"))]
mod android;
#[cfg(any(target_os = "android", feature = "android-jni-test"))]
mod android_editor;
#[cfg(any(target_os = "android", feature = "android-jni-test"))]
mod android_input;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistent_control_and_prediction() {
        unsafe {
            let directory = tempfile::tempdir().unwrap();
            let path = CString::new(directory.path().to_str().unwrap()).unwrap();
            let handle = meltype_create_at(path.as_ptr());
            assert!(!handle.is_null());
            let request =
                CString::new(r#"{"op":"dictionary_set","reading":"めるたいぷ","word":"Meltype"}"#)
                    .unwrap();
            let result: serde_json::Value =
                serde_json::from_str(&take(meltype_control(handle, request.as_ptr())).unwrap())
                    .unwrap();
            assert_eq!(result["saved"], true);
            for c in "meru".chars() {
                let result =
                    meltype_handle_key(handle, 0, c as i32, 0, std::ptr::null(), std::ptr::null());
                meltype_free(result.cast());
            }
            let result: serde_json::Value =
                serde_json::from_str(&take(meltype_select_prediction(handle, 0)).unwrap()).unwrap();
            assert_eq!(result["commits"][0]["text"], "Meltype");
            meltype_destroy(handle);
            let reopened = meltype_create_at(path.as_ptr());
            assert!(!reopened.is_null());
            let request = CString::new(r#"{"op":"dictionary_list"}"#).unwrap();
            let result: serde_json::Value =
                serde_json::from_str(&take(meltype_control(reopened, request.as_ptr())).unwrap())
                    .unwrap();
            assert_eq!(result["めるたいぷ"][0], "Meltype");
            meltype_destroy(reopened);
        }
    }
    #[test]
    fn lifecycle_and_unicode() {
        unsafe {
            let handle = meltype_create();
            assert!(!handle.is_null());
            for c in "konnichiha".chars() {
                let p =
                    meltype_handle_key(handle, 0, c as i32, 0, std::ptr::null(), std::ptr::null());
                assert!(!p.is_null());
                meltype_free(p.cast());
            }
            let p = meltype_commit(handle);
            let value: serde_json::Value = serde_json::from_str(&read(p).unwrap()).unwrap();
            assert_eq!(value["commits"][0]["text"], "こんにちは");
            meltype_free(p.cast());
            meltype_destroy(handle);
            meltype_destroy(handle);
            assert!(meltype_commit(handle).is_null());
            assert!(meltype_commit(std::ptr::null_mut()).is_null());
        }
    }
}
