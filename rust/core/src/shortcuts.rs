// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Shared configured shortcut parsing; modifier bits match the input ABI.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcut {
    pub vk: i32,
    pub modifiers: i32,
}
pub fn key_name(token: &str) -> Option<i32> {
    let token = token.to_ascii_lowercase();
    Some(match token.as_str() {
        "space" => 0x20,
        "tab" => 9,
        "enter" => 13,
        "esc" => 27,
        "backquote" | "grave" => 0xc0,
        "muhenkan" => 0x1d,
        "henkan" => 0x1c,
        "kanji" => 0x19,
        "capslock" => 0x14,
        _ if token.len() == 1 && token.bytes().all(|b| b.is_ascii_alphanumeric()) => {
            token.as_bytes()[0].to_ascii_uppercase() as i32
        }
        _ if (2..=3).contains(&token.len()) && token.starts_with('f') => {
            let n = token[1..].parse::<i32>().ok()?;
            if !(1..=12).contains(&n) {
                return None;
            }
            0x70 + n - 1
        }
        _ => return None,
    })
}
pub fn parse(text: &str) -> Option<Shortcut> {
    let mut modifiers = 0;
    let mut vk = None;
    for token in text.split('+').map(str::trim).filter(|t| !t.is_empty()) {
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= 2,
            "alt" => modifiers |= 4,
            "shift" => modifiers |= 1,
            "win" | "windows" => modifiers |= 8,
            _ => {
                if vk.is_some() {
                    return None;
                }
                vk = Some(key_name(token)?);
            }
        }
    }
    Some(Shortcut { vk: vk?, modifiers })
}
pub fn parse_list(text: &str) -> Vec<Shortcut> {
    text.split([';', ',', '\n']).filter_map(parse).collect()
}
/// User Control mappings take priority over built-in assignments.
pub fn control_bindings(text: &str) -> Vec<(i32, i32)> {
    let mut result = vec![];
    for part in text.split([';', ',', '\n']) {
        let Some((letter, key)) = part.split_once('=') else {
            continue;
        };
        let letter = letter.trim();
        if letter.len() != 1 || !letter.as_bytes()[0].is_ascii_alphabetic() {
            continue;
        }
        let key = match key.trim().to_ascii_lowercase().as_str() {
            "left" => 0x25,
            "right" => 0x27,
            "up" => 0x26,
            "down" => 0x28,
            "home" => 0x24,
            "end" => 0x23,
            "delete" => 0x2e,
            "back" => 8,
            key => match key_name(key) {
                Some(v) if matches!(v, 9 | 13 | 27 | 0x75..=0x79) => v,
                _ => continue,
            },
        };
        let letter = letter.as_bytes()[0].to_ascii_uppercase() as i32;
        if let Some((_, old)) = result.iter_mut().find(|(k, _)| *k == letter) {
            *old = key;
        } else {
            result.push((letter, key));
        }
    }
    result
}
