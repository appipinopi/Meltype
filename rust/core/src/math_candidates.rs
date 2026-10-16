// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Contextual arithmetic symbols from the upstream composition controller.
fn digits(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_digit() || ('０'..='９').contains(&c))
}
fn shape(reading: &str) -> Option<(&str, &str, &str)> {
    for op in [
        "かける",
        "わる",
        "たす",
        "ひく",
        "ぷらす",
        "まいなす",
        "いこーる",
    ] {
        if let Some((a, b)) = reading.split_once(op) {
            if digits(a) && digits(b) {
                return Some((a, op, b));
            }
        }
    }
    None
}
/// Returns the symbol and whether it should precede the ordinary candidate.
pub fn candidate(reading: &str, before: &str) -> Option<(String, bool)> {
    let (a, op, b) = shape(reading)?;
    let last = if a.is_empty() { before } else { a }.chars().last()?;
    if !last.is_ascii_digit() && !('０'..='９').contains(&last) {
        return None;
    }
    let full = ('０'..='９').contains(&last);
    let symbol = match op {
        "かける" => "×",
        "わる" => "÷",
        "たす" | "ぷらす" => {
            if full {
                "＋"
            } else {
                "+"
            }
        }
        "ひく" | "まいなす" => {
            if full {
                "－"
            } else {
                "-"
            }
        }
        _ => {
            if full {
                "＝"
            } else {
                "="
            }
        }
    };
    Some((format!("{a}{symbol}{b}"), matches!(op, "かける" | "わる")))
}
pub fn augment(reading: &str, before: &str, candidates: &mut Vec<String>) {
    if let Some((symbol, preferred)) = candidate(reading, before) {
        candidates.retain(|c| c != &symbol);
        candidates.insert(
            if preferred {
                0
            } else {
                1.min(candidates.len())
            },
            symbol,
        );
    }
}
pub fn is_choice(reading: &str, text: &str) -> bool {
    shape(reading).is_some() && text.contains(['×', '÷', '+', '＋', '-', '－', '=', '＝'])
}
