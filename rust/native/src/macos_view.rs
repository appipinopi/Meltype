// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! UTF-16 composition decoration plan; the host maps styles to Carbon marks.
use serde::Deserialize;
use std::ffi::c_void;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct View {
    text: String,
    converting: bool,
    clauses: Vec<String>,
    selected_clause: i64,
}
type Host = unsafe extern "C" fn(*mut c_void, u32, u64, u64, u32) -> i32;
fn plan(view: &View) -> Vec<(u64, u64, u32)> {
    let length = view.text.encode_utf16().count() as u64;
    let mut marks = Vec::new();
    if view.converting && !view.clauses.is_empty() {
        let mut location = 0u64;
        for (index, clause) in view.clauses.iter().enumerate() {
            let size = clause.encode_utf16().count() as u64;
            if size > 0 && location.checked_add(size).is_some_and(|end| end <= length) {
                marks.push((
                    location,
                    size,
                    if index as i64 == view.selected_clause {
                        2
                    } else {
                        1
                    },
                ));
            }
            location = location.saturating_add(size);
        }
    } else if length > 0 {
        marks.push((0, length, 0));
    }
    marks
}
/// Ops: mark(0: location/length/style; raw=0,converted=1,selected=2),
/// publish(1: selection location at text's UTF-16 end, length=0).
/// Complete validation/planning precedes callbacks; nonpositive return stops.
/// # Safety
/// input is readable for length bytes; callback/context remain live, and host
/// callbacks must not throw across the ABI.
#[no_mangle]
pub unsafe extern "C" fn meltype_macos_composition_utf8(
    input: *const u8,
    length: u32,
    context: *mut c_void,
    host: Option<Host>,
) -> i32 {
    let Some(host) = host else {
        return 0;
    };
    if input.is_null() {
        return 0;
    }
    let Ok(view) =
        serde_json::from_slice::<View>(std::slice::from_raw_parts(input, length as usize))
    else {
        return 0;
    };
    let marks = plan(&view);
    let length = view.text.encode_utf16().count() as u64;
    for (location, size, style) in marks {
        if host(context, 0, location, size, style) <= 0 {
            return 0;
        }
    }
    i32::from(host(context, 1, length, 0, 0) > 0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marks_use_utf16_and_ignore_outside_or_empty_clauses() {
        let mut view = View {
            text: "前😀後".into(),
            converting: true,
            clauses: vec!["前".into(), "😀".into(), "後".into()],
            selected_clause: 1,
        };
        assert_eq!(plan(&view), vec![(0, 1, 1), (1, 2, 2), (3, 1, 1)]);
        view.clauses = vec!["".into(), "前😀後外".into()];
        assert!(plan(&view).is_empty());
        view.converting = false;
        assert_eq!(plan(&view), vec![(0, 4, 0)]);
    }
}
