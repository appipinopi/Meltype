// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! C ABI for the existing macOS/Linux frontends and native embedding.
//! Sessions use opaque integer tokens. Stale handles fail without dereferencing
//! freed memory. Returned strings use malloc/free, including Swift callbacks.
use meltype_core::Session;
mod tip;
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
        all.insert(id, Session::default());
        id as *mut c_void
    })
    .unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn meltype_destroy(handle: *mut c_void) {
    if let Ok(mut all) = sessions().lock() {
        all.remove(&(handle as usize));
    }
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
    _after: *const c_char,
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
                        && !s.code_input
                        && modifiers & 14 == 0
                })
                .map(Session::reading)
        });
        let mut external = Vec::new();
        if let Some(reading) = reading {
            if let Ok(mut mozc) = MOZC.lock() {
                if let Some(backend) = mozc.as_mut() {
                    if let Ok(values) =
                        backend.candidates(&reading, &read(before).unwrap_or_default())
                    {
                        external.extend(values);
                    } else {
                        *mozc = None;
                    }
                }
            }
            let callbacks = CALLBACKS.lock().map(|c| *c).unwrap_or_default();
            let input = CString::new(reading.clone()).unwrap();
            if let Some(callback) = callbacks.clauses {
                if let Some(text) = take(callback(input.as_ptr(), before)) {
                    let clauses: Option<Vec<_>> =
                        text.lines().map(|line| line.split_once('\t')).collect();
                    if let Some(clauses) = clauses {
                        if clauses.iter().map(|(r, _)| *r).collect::<String>() == reading {
                            external.push(clauses.iter().map(|(_, t)| *t).collect::<String>());
                        }
                    }
                }
            }
            if let Some(callback) = callbacks.candidates {
                if let Some(text) = take(callback(input.as_ptr())) {
                    external.extend(text.lines().filter(|s| !s.is_empty()).map(str::to_owned));
                }
            }
        }
        run(handle, |s| {
            if !external.is_empty() {
                s.begin_conversion(external);
                s.result(true)
            } else {
                s.key(vk, ch, modifiers)
            }
        })
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn meltype_commit(handle: *mut c_void) -> *mut c_char {
    run(handle, Session::commit)
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
// This engine never deletes surrounding committed text.
#[no_mangle]
pub extern "C" fn meltype_set_can_delete(_handle: *mut c_void, _enabled: i32) {}
// NULL tells existing frontends to apply their regular mode-switch behavior.
#[no_mangle]
pub extern "C" fn meltype_role_key(_handle: *mut c_void, _vk: i32) -> *mut c_char {
    std::ptr::null_mut()
}
#[no_mangle]
pub extern "C" fn meltype_version() -> *mut c_char {
    allocate(concat!(env!("CARGO_PKG_VERSION"), "-rust-preview"))
}
#[no_mangle]
pub extern "C" fn meltype_data_directory() -> *mut c_char {
    let base = std::env::var_os("MELTYPE_RUST_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let base = if cfg!(target_os = "windows") {
                std::env::var_os("LOCALAPPDATA")
            } else {
                std::env::var_os("XDG_DATA_HOME").or_else(|| {
                    std::env::var_os("HOME").map(|p| {
                        std::path::PathBuf::from(p)
                            .join(".local/share")
                            .into_os_string()
                    })
                })
            };
            std::path::PathBuf::from(base.unwrap_or_else(|| ".".into())).join("MeltypeRust")
        });
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

#[cfg(target_os = "android")]
mod android;

#[cfg(test)]
mod tests {
    use super::*;
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
