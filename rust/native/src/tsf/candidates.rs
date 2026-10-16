// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Non-activating TSF candidate/meaning windows. Opaque tokens and thread-local
//! Rc ownership keep reentrant click/hide/destroy calls from touching freed state.
mod graphics;
mod paint;
use graphics::*;
use serde::Deserialize;
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{c_char, c_void, CStr},
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        OnceLock,
    },
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::*,
        Graphics::{
            Direct2D::{Common::*, *},
            DirectWrite::*,
            Gdi::*,
        },
        System::LibraryLoader::*,
        UI::{
            Controls::WM_MOUSELEAVE, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*,
        },
    },
};

const CLASS: PCWSTR = w!("MeltypeRustCandidateWindow");
const MEANING_CLASS: PCWSTR = w!("MeltypeRustMeaningWindow");
type Select = unsafe extern "C" fn(*mut c_void, i32, i32);
#[derive(Default, Deserialize)]
#[serde(default)]
struct View {
    candidates: Vec<String>,
    notes: Vec<String>,
    selected: i32,
    suggestion: String,
    meaning: String,
    prediction: bool,
    typed: String,
}
struct State {
    id: usize,
    callback: Option<Select>,
    context: *mut c_void,
    graphics: Graphics,
    window: HWND,
    meaning_window: HWND,
    target: Option<ID2D1HwndRenderTarget>,
    meaning_target: Option<ID2D1HwndRenderTarget>,
    view: View,
    anchor: RECT,
    first: usize,
    hover: i32,
    scale: f32,
    width: f32,
    meaning_key: String,
}
thread_local! { static WINDOWS: RefCell<HashMap<usize, Rc<RefCell<State>>>> = RefCell::new(HashMap::new()); }
static NEXT: AtomicUsize = AtomicUsize::new(1);
fn state(id: usize) -> Option<Rc<RefCell<State>>> {
    WINDOWS
        .try_with(|m| m.try_borrow().ok().and_then(|m| m.get(&id).cloned()))
        .ok()
        .flatten()
}
fn with<R>(id: usize, f: impl FnOnce(&mut State) -> R) -> Option<R> {
    let state = state(id)?;
    let mut state = state.try_borrow_mut().ok()?;
    Some(f(&mut state))
}
fn guard<R: Default>(f: impl FnOnce() -> R) -> R {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_default()
}
unsafe fn module() -> HINSTANCE {
    let mut value = HMODULE::default();
    let _ = GetModuleHandleExW(
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        PCWSTR(window_proc as *const () as *const u16),
        &mut value,
    );
    HINSTANCE(value.0)
}
unsafe fn classes() -> bool {
    static CLASSES: OnceLock<bool> = OnceLock::new();
    *CLASSES.get_or_init(|| {
        for (name, procedure) in [
            (
                CLASS,
                window_proc as unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
            ),
            (MEANING_CLASS, meaning_proc),
        ] {
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(procedure),
                hInstance: module(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: name,
                ..Default::default()
            };
            if RegisterClassExW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return false;
            }
        }
        true
    })
}
impl State {
    fn header(&self) -> f32 {
        (if self.view.typed.is_empty() { 0. } else { 22. })
            + if self.view.suggestion.is_empty() {
                0.
            } else {
                32. + if self.view.candidates.is_empty() {
                    0.
                } else {
                    5.
                }
            }
    }
    fn rows(&self) -> usize {
        self.view.candidates.len().saturating_sub(self.first).min(9)
    }
    fn paged(&self) -> bool {
        self.view.candidates.len() > 9
    }
    fn height(&self) -> f32 {
        8. + self.header()
            + if self.paged() {
                9. * 32. + 24.
            } else {
                self.view.candidates.len() as f32 * 32.
            }
    }
    fn hit(&self, y: f32) -> i32 {
        let top = 4. + self.header();
        if y < top {
            return -1;
        }
        let row = ((y - top) / 32.) as usize;
        if row < self.rows() {
            (self.first + row) as i32
        } else {
            -1
        }
    }
    unsafe fn create_window(&self, meaning: bool) -> Option<HWND> {
        let window = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            if meaning { MEANING_CLASS } else { CLASS },
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            module(),
            Some(self.id as *const c_void),
        )
        .ok()?;
        prepare(window);
        Some(window)
    }
    unsafe fn measure(&self) -> f32 {
        let candidate = self.graphics.format(15.);
        let note = self.graphics.format(12.);
        let mut widest = 0f32;
        for (i, text) in self.view.candidates.iter().enumerate() {
            let mut width = self.graphics.width(candidate.as_ref(), text);
            if let Some(text) = self.view.notes.get(i).filter(|s| !s.is_empty()) {
                width += 16. + self.graphics.width(note.as_ref(), text);
            }
            widest = widest.max(width);
        }
        let mut width = 8. + 10. + 26. + widest + 16.;
        if !self.view.suggestion.is_empty() {
            width = width.max(
                8. + 12.
                    + self
                        .graphics
                        .width(candidate.as_ref(), &self.view.suggestion)
                    + 16.,
            );
        }
        if !self.view.typed.is_empty() {
            width =
                width.max(8. + 12. + self.graphics.width(note.as_ref(), &self.view.typed) + 16.);
        }
        width.clamp(168., 520.)
    }
    unsafe fn show(&mut self, view: View, anchor: RECT) -> bool {
        if self.window.is_invalid() {
            let Some(window) = self.create_window(false) else {
                return false;
            };
            self.window = window;
        }
        let scale = GetDpiForWindow(self.window) as f32 / 96.;
        let scale = if scale <= 0. { 1. } else { scale };
        let same = IsWindowVisible(self.window).as_bool()
            && self.anchor == anchor
            && self.scale == scale
            && self.view.candidates == view.candidates
            && self.view.notes == view.notes
            && self.view.suggestion == view.suggestion
            && self.view.prediction == view.prediction
            && self.view.typed == view.typed;
        self.first = (view.selected.max(0) as usize / 9 * 9).min(self.view_page_end(&view));
        self.view = view;
        self.anchor = anchor;
        self.scale = scale;
        if !same {
            self.width = self.measure();
            self.target = None;
            for _ in 0..2 {
                let width = (self.width * self.scale + 0.5) as i32;
                let height = (self.height() * self.scale + 0.5) as i32;
                let inset = if self.view.candidates.is_empty() {
                    16.
                } else {
                    40.
                };
                let mut x = anchor.left - (inset * self.scale) as i32;
                let mut y = anchor.bottom + (4. * self.scale) as i32;
                place(&mut x, &mut y, width, height, &anchor);
                let _ = SetWindowPos(
                    self.window,
                    HWND_TOPMOST,
                    x,
                    y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                let moved = GetDpiForWindow(self.window) as f32 / 96.;
                if moved <= 0. || moved == self.scale {
                    break;
                }
                self.scale = moved;
            }
        }
        let _ = InvalidateRect(self.window, None, false);
        self.schedule_meaning();
        true
    }
    fn view_page_end(&self, view: &View) -> usize {
        view.candidates.len().saturating_sub(1) / 9 * 9
    }
    unsafe fn schedule_meaning(&mut self) {
        let key = if self.view.meaning.is_empty() || self.view.selected < 0 {
            String::new()
        } else {
            format!("{}\n{}", self.view.selected, self.view.meaning)
        };
        if self.meaning_key == key {
            if !self.meaning_window.is_invalid() && IsWindowVisible(self.meaning_window).as_bool() {
                self.place_meaning();
            }
            return;
        }
        self.meaning_key = key;
        let _ = KillTimer(self.window, 1);
        let _ = ShowWindow(self.meaning_window, SW_HIDE);
        if !self.meaning_key.is_empty() {
            SetTimer(self.window, 1, 1500, None);
        }
    }
    unsafe fn hide(&mut self) {
        if !self.window.is_invalid() {
            let _ = KillTimer(self.window, 1);
            let _ = ShowWindow(self.window, SW_HIDE);
        }
        if !self.meaning_window.is_invalid() {
            let _ = ShowWindow(self.meaning_window, SW_HIDE);
        }
        self.meaning_key.clear();
        self.hover = -1;
    }
}
impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            self.callback = None;
            self.target = None;
            self.meaning_target = None;
            if !self.meaning_window.is_invalid() {
                let _ = DestroyWindow(self.meaning_window);
            }
            if !self.window.is_invalid() {
                let _ = DestroyWindow(self.window);
            }
        }
    }
}
unsafe fn token(window: HWND, message: u32, parameter: LPARAM) -> usize {
    if message == WM_NCCREATE {
        let creation = &*(parameter.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(window, GWLP_USERDATA, creation.lpCreateParams as _);
    }
    GetWindowLongPtrW(window, GWLP_USERDATA) as usize
}
unsafe extern "system" fn window_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let id = token(window, message, l);
        if id == 0 || state(id).is_none() {
            return DefWindowProcW(window, message, w, l);
        }
        match message {
            WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
            WM_ERASEBKGND => return LRESULT(1),
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                BeginPaint(window, &mut paint);
                with(id, |s| s.paint());
                let _ = EndPaint(window, &paint);
                return LRESULT(0);
            }
            WM_MOUSEMOVE => {
                with(id, |s| {
                    let hit = s.hit(((l.0 >> 16) as i16) as f32 / s.scale);
                    if hit != s.hover {
                        s.hover = hit;
                        let _ = InvalidateRect(window, None, false);
                        let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: window,
                            dwHoverTime: 0,
                        });
                    }
                });
                return LRESULT(0);
            }
            WM_MOUSELEAVE => {
                with(id, |s| {
                    s.hover = -1;
                    let _ = InvalidateRect(window, None, false);
                });
                return LRESULT(0);
            }
            WM_LBUTTONUP => {
                let selection = with(id, |s| {
                    let hit = s.hit(((l.0 >> 16) as i16) as f32 / s.scale);
                    (hit >= 0).then_some((s.callback, s.context, hit, i32::from(s.view.prediction)))
                })
                .flatten();
                // No RefCell borrow is held while the host can hide/destroy us.
                if let Some((Some(callback), context, index, prediction)) = selection {
                    callback(context, index, prediction);
                }
                return LRESULT(0);
            }
            WM_TIMER => {
                if w.0 == 1 {
                    with(id, |s| s.show_meaning());
                }
                return LRESULT(0);
            }
            WM_DPICHANGED => return LRESULT(0),
            WM_NCDESTROY => {
                with(id, |s| {
                    s.window = HWND::default();
                    s.target = None;
                });
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            }
            _ => {}
        }
        DefWindowProcW(window, message, w, l)
    }))
    .unwrap_or_else(|_| DefWindowProcW(window, message, w, l))
}
unsafe extern "system" fn meaning_proc(
    window: HWND,
    message: u32,
    w: WPARAM,
    l: LPARAM,
) -> LRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let id = token(window, message, l);
        if id != 0 && state(id).is_some() {
            match message {
                WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
                WM_ERASEBKGND => return LRESULT(1),
                WM_PAINT => {
                    let mut paint = PAINTSTRUCT::default();
                    BeginPaint(window, &mut paint);
                    with(id, |s| s.paint_meaning());
                    let _ = EndPaint(window, &paint);
                    return LRESULT(0);
                }
                WM_NCDESTROY => {
                    with(id, |s| {
                        s.meaning_window = HWND::default();
                        s.meaning_target = None;
                    });
                    SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                }
                _ => {}
            }
        }
        DefWindowProcW(window, message, w, l)
    }))
    .unwrap_or_else(|_| DefWindowProcW(window, message, w, l))
}

/// Creates a thread-affine candidate window token; the callback may reenter.
#[no_mangle]
pub extern "C" fn meltype_tsf_candidate_create(
    context: *mut c_void,
    callback: Option<Select>,
) -> *mut c_void {
    guard(|| unsafe {
        let Ok(graphics) = Graphics::new() else {
            return ptr::null_mut();
        };
        if !classes() {
            return ptr::null_mut();
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let state = State {
            id,
            callback,
            context,
            graphics,
            window: HWND::default(),
            meaning_window: HWND::default(),
            target: None,
            meaning_target: None,
            view: View::default(),
            anchor: RECT::default(),
            first: 0,
            hover: -1,
            scale: 1.,
            width: 168.,
            meaning_key: String::new(),
        };
        WINDOWS.with(|m| m.borrow_mut().insert(id, Rc::new(RefCell::new(state))));
        id as *mut c_void
    })
}
/// # Safety
/// `json` is a NUL-terminated UTF-8 view; `anchor` points to a readable RECT.
/// Call on the creating thread. Stale/foreign tokens return 0.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_candidate_show(
    handle: *mut c_void,
    json: *const c_char,
    anchor: *const RECT,
) -> i32 {
    guard(|| {
        if json.is_null() || anchor.is_null() {
            return 0;
        }
        let Ok(view) = serde_json::from_slice::<View>(CStr::from_ptr(json).to_bytes()) else {
            return 0;
        };
        with(handle as usize, |s| s.show(view, *anchor)).is_some_and(|v| v) as i32
    })
}
#[no_mangle]
pub extern "C" fn meltype_tsf_candidate_hide(handle: *mut c_void) {
    guard(|| {
        with(handle as usize, |s| unsafe { s.hide() });
    });
}
#[no_mangle]
pub extern "C" fn meltype_tsf_candidate_visible(handle: *mut c_void) -> i32 {
    guard(|| {
        with(handle as usize, |s| unsafe {
            IsWindowVisible(s.window).as_bool() as i32
        })
        .unwrap_or(0)
    })
}
#[no_mangle]
pub extern "C" fn meltype_tsf_candidate_destroy(handle: *mut c_void) {
    guard(|| {
        let state = WINDOWS.with(|m| m.borrow_mut().remove(&(handle as usize)));
        drop(state);
    });
}
