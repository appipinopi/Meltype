// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Project core replies and anchor candidates to the selected TSF clause.
use super::*;
use windows::{
    core::Interface,
    Win32::{System::Threading::GetCurrentThreadId, UI::TextServices::*},
};

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ReplyView {
    candidates: Vec<String>,
    notes: Vec<String>,
    predictions: Vec<String>,
    selected_index: i32,
    selected_prediction: i32,
    suggestion: String,
    meaning: String,
    typed: String,
    text: String,
    converting: bool,
    clauses: Vec<String>,
    selected_clause: i32,
}
impl ReplyView {
    fn project(&self) -> Option<View> {
        let typed = if self.typed == self.text {
            String::new()
        } else {
            self.typed.clone()
        };
        if !(self.converting && self.candidates.len() > 1
            || !self.suggestion.is_empty()
            || !self.converting && !self.predictions.is_empty()
            || !typed.is_empty())
        {
            return None;
        }
        let mut view = View {
            candidates: self.candidates.clone(),
            notes: self.notes.clone(),
            selected: self.selected_index,
            suggestion: self.suggestion.clone(),
            meaning: self.meaning.clone(),
            typed,
            prediction: false,
        };
        if self.converting && view.candidates.len() <= 1 {
            view.candidates.clear();
            view.meaning.clear();
        }
        if !self.converting {
            view.candidates = self.predictions.clone();
            view.notes.clear();
            view.meaning.clear();
            view.selected = if self.predictions.is_empty() {
                -1
            } else {
                self.selected_prediction
            };
            view.prediction = !self.predictions.is_empty();
        }
        Some(view)
    }
}
unsafe fn text_anchor(
    context: *mut c_void,
    composition: *mut c_void,
    cookie: u32,
    view: &ReplyView,
) -> Option<RECT> {
    let context = ITfContext::from_raw_borrowed(&context)?.clone();
    let composition = ITfComposition::from_raw_borrowed(&composition)?.clone();
    let range = composition.GetRange().ok()?.Clone().ok()?;
    if view.converting {
        if let Some(selected) = usize::try_from(view.selected_clause)
            .ok()
            .filter(|&i| i < view.clauses.len())
        {
            let offset = view.clauses[..selected]
                .iter()
                .try_fold(0usize, |n, s| n.checked_add(s.encode_utf16().count()))?;
            let end = offset.checked_add(view.clauses[selected].encode_utf16().count())?;
            let (offset, end) = (i32::try_from(offset).ok()?, i32::try_from(end).ok()?);
            range.Collapse(cookie, TF_ANCHOR_START).ok()?;
            let mut moved = 0;
            range.ShiftEnd(cookie, end, &mut moved, ptr::null()).ok()?;
            if moved != end {
                return None;
            }
            range
                .ShiftStart(cookie, offset, &mut moved, ptr::null())
                .ok()?;
            if moved != offset {
                return None;
            }
        }
    }
    let active = context.GetActiveView().ok()?;
    let (mut rect, mut clipped) = (RECT::default(), BOOL::default());
    active
        .GetTextExt(cookie, &range, &mut rect, &mut clipped)
        .ok()?;
    if rect.left == 0 && rect.top == 0 && rect.right == 0 && rect.bottom == 0 {
        None
    } else {
        Some(rect)
    }
}
unsafe fn fallback_anchor() -> RECT {
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    if GetGUIThreadInfo(GetCurrentThreadId(), &mut info).is_ok() && !info.hwndCaret.is_invalid() {
        let mut point = POINT {
            x: info.rcCaret.left,
            y: info.rcCaret.top,
        };
        let _ = ClientToScreen(info.hwndCaret, &mut point);
        RECT {
            left: point.x,
            top: point.y,
            right: point.x.saturating_add(1),
            bottom: point
                .y
                .saturating_add(info.rcCaret.bottom.saturating_sub(info.rcCaret.top)),
        }
    } else {
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        RECT {
            left: point.x,
            top: point.y,
            right: point.x.saturating_add(1),
            bottom: point.y.saturating_add(16),
        }
    }
}
/// Resolve the selected clause rectangle, with caret/cursor fallback. Return 1
/// for a TSF rectangle and 0 for fallback (also written to out). Invalid JSON or
/// null out returns -1 without touching the output. No document text is changed.
/// # Safety
/// Borrowed context/composition on their TSF thread, valid read cookie, UTF-8
/// NUL-terminated reply view and writable RECT. Context/composition may be null.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_candidate_anchor(
    context: *mut c_void,
    composition: *mut c_void,
    cookie: u32,
    json: *const c_char,
    out: *mut RECT,
) -> i32 {
    guard(|| {
        if json.is_null() || out.is_null() {
            return -1;
        }
        let Ok(view) = serde_json::from_slice::<ReplyView>(CStr::from_ptr(json).to_bytes()) else {
            return -1;
        };
        if let Some(rect) = text_anchor(context, composition, cookie, &view) {
            out.write(rect);
            1
        } else {
            out.write(fallback_anchor());
            0
        }
    })
}
/// Apply a core reply to an existing candidate token, hiding irrelevant views.
/// # Safety
/// On the token/context/composition thread; borrowed live COM interfaces (or
/// null), valid cookie and NUL-terminated UTF-8 core reply view.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_candidate_show_tsf(
    handle: *mut c_void,
    context: *mut c_void,
    composition: *mut c_void,
    cookie: u32,
    json: *const c_char,
) -> i32 {
    guard(|| {
        if state(handle as usize).is_none() || json.is_null() {
            return 0;
        }
        let Ok(reply) = serde_json::from_slice::<ReplyView>(CStr::from_ptr(json).to_bytes()) else {
            return 0;
        };
        let Some(view) = reply.project() else {
            with(handle as usize, |s| s.hide());
            return 1;
        };
        // Host COM calls may reenter and destroy the token; with() rechecks it.
        let anchor =
            text_anchor(context, composition, cookie, &reply).unwrap_or_else(|| fallback_anchor());
        with(handle as usize, |s| s.show(view, anchor)).is_some_and(|v| v) as i32
    })
}
