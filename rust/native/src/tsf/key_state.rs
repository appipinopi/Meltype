// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Thread-affine privacy cache and bounded reinjection bookkeeping.
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};
use windows::{
    core::{IUnknown, Interface},
    Win32::{
        Foundation::GetLastError,
        System::SystemInformation::GetTickCount64,
        UI::{
            Input::KeyboardAndMouse::*, TextServices::ITfContext,
            WindowsAndMessaging::GetMessageExtraInfo,
        },
    },
};
const MARKER: usize = 0x4d4c5450;
#[derive(Clone, Copy, Default)]
struct Pending {
    key: u32,
    time: u64,
    serial: u64,
}
#[derive(Default)]
struct Queue {
    pending: [Pending; 4],
    marked: bool,
    guard_key: u32,
    guard_time: u64,
    guard_count: u32,
    serial: u64,
}
impl Queue {
    fn observe(&mut self, key: u32, marked: bool, time: u64) -> bool {
        for pending in &mut self.pending {
            if pending.key != 0 && time.wrapping_sub(pending.time) > 2000 {
                pending.key = 0;
                self.marked = false;
            }
            if pending.key == 0 || pending.key != key {
                continue;
            }
            if marked {
                self.marked = true;
            } else if self.marked {
                continue;
            }
            let ours = marked || time.wrapping_sub(pending.time) < 500;
            pending.key = 0;
            return ours;
        }
        false
    }
    fn prepare(&mut self, key: u32, time: u64) -> Option<(usize, u64)> {
        let previous = self
            .pending
            .iter()
            .any(|p| p.key == key && time.wrapping_sub(p.time) < 2000);
        if !previous || key != self.guard_key || time.wrapping_sub(self.guard_time) > 1000 {
            self.guard_key = key;
            self.guard_time = time;
            self.guard_count = 0;
        }
        if previous {
            self.guard_count = self.guard_count.saturating_add(1);
            if self.guard_count > 3 {
                return None;
            }
        }
        let mut index = 0;
        for (i, p) in self.pending.iter().enumerate() {
            if p.key == 0 {
                index = i;
                break;
            }
            if p.time < self.pending[index].time {
                index = i;
            }
        }
        self.serial = self.serial.wrapping_add(1);
        self.pending[index] = Pending {
            key,
            time,
            serial: self.serial,
        };
        Some((index, self.serial))
    }
    fn failed(&mut self, index: usize, serial: u64) {
        if self.pending[index].serial == serial {
            self.pending[index].key = 0;
        }
    }
}
struct Cache {
    context: ITfContext,
    key: u32,
    time: u64,
    result: i32,
    private: i32,
}
#[derive(Default)]
struct State {
    cache: Option<Rc<Cache>>,
    epoch: u64,
    queue: Queue,
}
impl State {
    fn reset(&mut self) -> Self {
        let next = Self {
            epoch: self.epoch.wrapping_add(1),
            queue: Queue {
                serial: self.queue.serial.wrapping_add(1),
                ..Default::default()
            },
            ..Default::default()
        };
        std::mem::replace(self, next)
    }
}
type Shared = Rc<RefCell<State>>;
thread_local! {static STATES:RefCell<HashMap<usize,Shared>>=RefCell::new(HashMap::new());}
static NEXT: AtomicUsize = AtomicUsize::new(1);
fn state(handle: *mut c_void) -> Option<Shared> {
    STATES
        .try_with(|m| {
            m.try_borrow()
                .ok()
                .and_then(|m| m.get(&(handle as usize)).cloned())
        })
        .ok()
        .flatten()
}
#[no_mangle]
pub extern "C" fn meltype_tsf_key_state_create() -> *mut c_void {
    catch_unwind(|| {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        STATES.with(|m| {
            m.borrow_mut()
                .insert(id, Rc::new(RefCell::new(State::default())))
        });
        id as *mut c_void
    })
    .unwrap_or(ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn meltype_tsf_key_state_reset(handle: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(state) = state(handle) {
            let old = state.borrow_mut().reset();
            drop(old);
        }
    }));
}
#[no_mangle]
pub extern "C" fn meltype_tsf_key_state_destroy(handle: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let state = STATES
            .try_with(|m| {
                m.try_borrow_mut()
                    .ok()
                    .and_then(|mut m| m.remove(&(handle as usize)))
            })
            .ok()
            .flatten();
        if let Some(state) = state {
            let old = state.borrow_mut().reset();
            drop(old);
        }
    }));
}
#[no_mangle]
pub extern "C" fn meltype_tsf_key_cache_clear(handle: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(state) = state(handle) {
            let old = {
                let mut s = state.borrow_mut();
                s.epoch = s.epoch.wrapping_add(1);
                s.cache.take()
            };
            drop(old);
        }
    }));
}
type Reader = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32;
/// Reuse a test-key result once for the same key/context within 200ms. A newer
/// reentrant request/reset prevents the outer result from replacing its cache.
/// # Safety
/// Borrowed live context and owner IUnknown on their TSF thread, writable private
/// output, callback/state/code valid until return; reader must never throw.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_key_secret(
    handle: *mut c_void,
    context: *mut c_void,
    key: u32,
    test: i32,
    owner: *mut c_void,
    lifetime: *mut c_void,
    reader: Option<Reader>,
    private: *mut i32,
) -> i32 {
    if private.is_null() {
        return 1;
    }
    private.write(0);
    catch_unwind(AssertUnwindSafe(|| {
        let (Some(state), Some(reader)) = (state(handle), reader) else {
            return 1;
        };
        let Some(_owner) = IUnknown::from_raw_borrowed(&lifetime).cloned() else {
            return 1;
        };
        let Some(context) = ITfContext::from_raw_borrowed(&context).cloned() else {
            return -1;
        };
        let time = GetTickCount64();
        let (old, epoch) = {
            let mut s = state.borrow_mut();
            s.epoch = s.epoch.wrapping_add(1);
            (s.cache.take(), s.epoch)
        };
        if test == 0 {
            if let Some(cache) = old.as_ref().filter(|c| {
                c.key == key
                    && c.context.as_raw() == context.as_raw()
                    && time.wrapping_sub(c.time) < 200
            }) {
                private.write(cache.private);
                return cache.result;
            }
        }
        let mut field = 0;
        let result = reader(owner, context.as_raw(), &mut field);
        private.write(field);
        if test != 0 {
            let cache = Rc::new(Cache {
                context,
                key,
                time,
                result,
                private: field,
            });
            let mut s = state.borrow_mut();
            if s.epoch == epoch {
                s.cache = Some(cache);
            }
        }
        result
    }))
    .unwrap_or(1)
}
#[no_mangle]
pub extern "C" fn meltype_tsf_is_reinjected(handle: *mut c_void, key: u32) -> i32 {
    i32::from(
        catch_unwind(AssertUnwindSafe(|| unsafe {
            let Some(state) = state(handle) else {
                return false;
            };
            let marked = GetMessageExtraInfo().0 as usize == MARKER;
            let value = state
                .borrow_mut()
                .queue
                .observe(key, marked, GetTickCount64());
            value
        }))
        .unwrap_or(false),
    )
}
type Sender = unsafe extern "C" fn(*mut c_void, *const INPUT, u32) -> u32;
/// Returns 1 sent, 2 suppressed loop, 0 transport failure, -1 invalid token/key.
/// An optional sender substitutes the transport (e.g. a controlled host); null
/// uses SendInput. Owner is retained across the transport and reentrant hooks.
/// # Safety
/// On the token's TSF thread, borrowed owner IUnknown, optional callback/state
/// valid until return. Sender borrows two INPUTs, returns sent count, never throws.
/// error is optional writable u32; Windows last-error is captured on failure.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_reinject(
    handle: *mut c_void,
    key: u32,
    lifetime: *mut c_void,
    owner: *mut c_void,
    sender: Option<Sender>,
    error: *mut u32,
) -> i32 {
    if !error.is_null() {
        error.write(0);
    }
    catch_unwind(AssertUnwindSafe(|| {
        let Some(state) = state(handle) else {
            return -1;
        };
        if key == 0 || key >= 256 {
            return -1;
        }
        let Some(_owner) = IUnknown::from_raw_borrowed(&lifetime).cloned() else {
            return -1;
        };
        let (index, serial) = {
            let mut state = state.borrow_mut();
            let Some((index, serial)) = state.queue.prepare(key, GetTickCount64()) else {
                return 2;
            };
            state.queue.pending[index].time = GetTickCount64();
            (index, serial)
        };
        let extended = matches!(key, 0x21..=0x28 | 0x2d | 0x2e);
        let inputs = [false, true].map(|up| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(key as u16),
                    wScan: MapVirtualKeyW(key, MAPVK_VK_TO_VSC) as u16,
                    dwFlags: (if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    }) | (if extended {
                        KEYEVENTF_EXTENDEDKEY
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    }),
                    dwExtraInfo: MARKER,
                    ..Default::default()
                },
            },
        });
        let count = match sender {
            Some(send) => send(owner, inputs.as_ptr(), 2),
            None => SendInput(&inputs, std::mem::size_of::<INPUT>() as i32),
        };
        let failure = if count != 2 { GetLastError().0 } else { 0 };
        if count != 2 {
            state.borrow_mut().queue.failed(index, serial);
            if !error.is_null() {
                error.write(failure);
            }
            return 0;
        }
        1
    }))
    .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marker_and_timeout_boundaries() {
        let mut q = Queue::default();
        q.prepare(65, 100);
        assert!(q.observe(65, false, 599));
        assert!(!q.observe(65, false, 599));
        q.prepare(65, 1000);
        assert!(!q.observe(65, false, 1500));
        q.prepare(65, 2000);
        assert!(q.observe(65, true, 4000));
        q.prepare(65, 5000);
        assert!(!q.observe(65, true, 7001));
    }
    #[test]
    fn recognized_marker_does_not_consume_unmarked_human_key() {
        let mut q = Queue::default();
        q.prepare(65, 100);
        assert!(q.observe(65, true, 101));
        q.prepare(65, 200);
        assert!(!q.observe(65, false, 201));
        assert!(q.observe(65, true, 202));
        q.prepare(66, 300);
        q.prepare(65, 2400);
        assert!(q.observe(65, false, 2401));
    }
    #[test]
    fn repeated_reinjection_guard_recovers_after_window() {
        let mut q = Queue::default();
        for t in 0..4 {
            assert!(q.prepare(65, t).is_some());
        }
        assert!(q.prepare(65, 4).is_none());
        assert!(q.prepare(65, 1000).is_none());
        assert!(q.prepare(65, 1001).is_some());
    }
    #[test]
    fn queue_eviction_and_failed_send_do_not_clear_reentrant_replacement() {
        let mut q = Queue::default();
        for key in 65..69 {
            q.prepare(key, key as u64);
        }
        let (slot, old) = q.prepare(69, 100).unwrap();
        assert!(!q.observe(65, true, 101));
        q.prepare(70, 101);
        q.prepare(71, 102);
        q.prepare(72, 103);
        q.prepare(73, 104);
        q.failed(slot, old);
        assert!(q.observe(73, true, 105));
    }
}
