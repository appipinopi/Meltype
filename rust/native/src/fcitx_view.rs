// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! fcitx panel policy. SDK objects remain behind synchronous host callbacks.
use serde_json::Value;
use std::ffi::c_void;
struct Event(u32, i32, String);
fn string<'a>(value: &'a Value, name: &str) -> &'a str {
    value.get(name).and_then(Value::as_str).unwrap_or("")
}
fn strings<'a>(value: &'a Value, name: &str) -> Vec<&'a str> {
    value
        .get(name)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}
fn integer(value: &Value, name: &str, fallback: i32) -> i32 {
    value
        .get(name)
        .and_then(Value::as_f64)
        .map_or(fallback, |v| v as i32)
}
fn plan(result: &Value) -> Vec<Event> {
    let Some(view) = result.get("view").filter(|v| v.is_object()) else {
        return vec![Event(8, 0, String::new())];
    };
    let mut events = vec![Event(0, 0, String::new())];
    let text = string(view, "text");
    let clauses = strings(view, "clauses");
    let converting = view.get("converting").and_then(Value::as_bool) == Some(true);
    let selected = integer(view, "selectedClause", -1);
    if converting && !clauses.is_empty() {
        for (index, clause) in clauses.iter().enumerate() {
            events.push(Event(
                1,
                1 | (i32::from(index as i32 == selected) << 1),
                (*clause).into(),
            ));
        }
    } else {
        events.push(Event(1, 1, text.into()));
    }
    events.push(Event(2, i32::from(!text.is_empty()), String::new()));
    let hint = string(view, "hint");
    let suggestion = string(view, "suggestion");
    let hint = if suggestion.is_empty() {
        hint.into()
    } else {
        format!("{suggestion}　{hint}")
    };
    if !hint.is_empty() {
        events.push(Event(3, 0, hint));
    }
    let candidates = strings(view, "candidates");
    if converting && candidates.len() > 1 {
        events.push(Event(4, 9, String::new()));
        for (index, candidate) in candidates.iter().enumerate() {
            events.push(Event(5, index as i32, (*candidate).into()));
        }
        let selected = integer(view, "selectedIndex", 0);
        let selected = if selected >= 0 && (selected as usize) < candidates.len() {
            selected
        } else {
            -1
        };
        events.push(Event(6, selected, String::new()));
    }
    events.push(Event(7, 0, String::new()));
    events
}
pub(crate) type Callback = unsafe extern "C" fn(*mut c_void, u32, i32, *const u8, u32) -> i32;
/// Emit reset(0), formatted segment(1: underline=1/highlight=2), preedit(2:
/// visible), auxiliary(3), list begin(4: page size), item(5: index), list end(6:
/// selected or -1), publish(7), or hide(8). Text is borrowed for one callback.
/// Complete JSON validation and planning precede the first callback. Returning
/// zero cancels traversal; the host must not throw across this ABI.
/// # Safety
/// input is readable for length bytes; callback/context remain valid throughout.
#[no_mangle]
pub unsafe extern "C" fn meltype_fcitx_view_utf8(
    input: *const u8,
    length: u32,
    context: *mut c_void,
    callback: Option<Callback>,
) -> i32 {
    let Some(callback) = callback else {
        return 0;
    };
    if input.is_null() {
        return 0;
    }
    let Ok(result) =
        serde_json::from_slice::<Value>(std::slice::from_raw_parts(input, length as usize))
    else {
        return 0;
    };
    for Event(kind, argument, text) in plan(&result) {
        if callback(context, kind, argument, text.as_ptr(), text.len() as u32) <= 0 {
            return 0;
        }
    }
    1
}
/// Apply the response in upstream order. Callback 9 queries a live expected
/// suffix (argument: UTF-16 units), returning SDK character count, zero for a
/// mismatch or negative on host failure. Operations 10/11/12 delete characters,
/// commit text, and reset the panel without notifications. Panel events 0..8
/// follow edits. A mismatch resets the core and hides without applying the view.
/// # Safety
/// Same buffer/callback requirements as meltype_fcitx_view_utf8. handle is an
/// opaque core session token; no session lock is held while callbacks execute.
#[no_mangle]
pub unsafe extern "C" fn meltype_fcitx_apply_utf8(
    handle: *mut c_void,
    input: *const u8,
    length: u32,
    context: *mut c_void,
    callback: Option<Callback>,
) -> i32 {
    let Some(callback) = callback else {
        return 0;
    };
    if input.is_null() {
        return 0;
    }
    let Ok(result) =
        serde_json::from_slice::<Value>(std::slice::from_raw_parts(input, length as usize))
    else {
        return 0;
    };
    // Validate and allocate the view before applying any editor mutation.
    let view = plan(&result);
    let emit = |kind, argument, text: &str| {
        callback(context, kind, argument, text.as_ptr(), text.len() as u32)
    };
    if let Some(commits) = result.get("commits").and_then(Value::as_array) {
        for edit in commits {
            let remove = integer(edit, "deleteBefore", 0);
            if remove > 0 {
                let count = emit(9, remove, string(edit, "expect"));
                if count < 0 {
                    return 0;
                }
                if count == 0 {
                    let reset = super::meltype_reset(handle);
                    super::meltype_free(reset.cast());
                    return i32::from(emit(8, 0, "") > 0);
                }
                if emit(8, 0, "") <= 0 || emit(10, count, "") <= 0 {
                    return 0;
                }
            } else if emit(12, 0, "") <= 0 {
                return 0;
            }
            let text = string(edit, "text");
            if !text.is_empty() && emit(11, 0, text) <= 0 {
                return 0;
            }
        }
    }
    for Event(kind, argument, text) in view {
        if emit(kind, argument, &text) <= 0 {
            return 0;
        }
    }
    1
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clauses_visibility_and_candidate_selection_keep_original_policy() {
        let events = plan(
            &serde_json::json!({"view":{"text":"かな", "clauses":["仮",false,"名"], "converting":true,"selectedClause":1,"candidates":["仮名",3,"かな"],"selectedIndex":99,"suggestion":"候補"}}),
        );
        assert_eq!(
            events.iter().map(|e| (e.0, e.1)).collect::<Vec<_>>(),
            vec![
                (0, 0),
                (1, 1),
                (1, 3),
                (2, 1),
                (3, 0),
                (4, 9),
                (5, 0),
                (5, 1),
                (6, -1),
                (7, 0)
            ]
        );
        assert_eq!(events[4].2, "候補　");
        assert_eq!(plan(&serde_json::json!({}))[0].0, 8);
    }
}
