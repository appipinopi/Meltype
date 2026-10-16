// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
use std::{collections::HashMap, sync::OnceLock};

fn table() -> &'static HashMap<&'static str, &'static str> {
    static TABLE: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    TABLE.get_or_init(|| {
        include_str!("romaji.tsv")
            .lines()
            .filter_map(|s| s.split_once('\t'))
            .collect()
    })
}

/// Incremental conversion. An incomplete consonant is preserved until commit.
pub fn hiragana(raw: &str, final_input: bool) -> String {
    let lower = raw.to_ascii_lowercase();
    let mut rest = lower.as_str();
    let mut out = String::new();
    while !rest.is_empty() {
        let mut chars = rest.chars();
        let first = chars.next().unwrap();
        let next = chars.next();
        if first.is_ascii_alphabetic()
            && first != 'n'
            && !"aiueo".contains(first)
            && next == Some(first)
        {
            out.push('っ');
            rest = &rest[1..];
            continue;
        }
        if first == 'n' {
            if next == Some('\'') {
                out.push('ん');
                rest = &rest[2..];
                continue;
            }
            if next == Some('n') {
                out.push('ん');
                let third = rest.chars().nth(2);
                rest = &rest[if third.is_some_and(|c| "aiueoy".contains(c)) {
                    1
                } else {
                    2
                }..];
                continue;
            }
            if next.is_some_and(|c| !"aiueoy".contains(c)) || (next.is_none() && final_input) {
                out.push('ん');
                rest = &rest[1..];
                continue;
            }
        }
        if let Some((spelling, kana)) = (1..=4)
            .rev()
            .filter_map(|n| rest.get(..n).and_then(|s| table().get(s).map(|k| (s, *k))))
            .next()
        {
            out.push_str(kana);
            rest = &rest[spelling.len()..];
            continue;
        }
        if !final_input && table().keys().any(|key| key.starts_with(rest)) {
            out.push_str(rest);
            break;
        }
        out.push(match first {
            ',' => '、',
            '.' => '。',
            '-' => 'ー',
            '[' => '「',
            ']' => '」',
            _ => first,
        });
        rest = &rest[first.len_utf8()..];
    }
    out
}

pub fn katakana(text: &str) -> String {
    text.chars()
        .map(|c| {
            if ('\u{3041}'..='\u{3096}').contains(&c) {
                char::from_u32(c as u32 + 0x60).unwrap()
            } else {
                c
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn romanization() {
        for (raw, expected) in [
            ("konnichiwa", "こんにちわ"),
            ("gakkou", "がっこう"),
            ("shin'you", "しんよう"),
            ("konnya", "こんにゃ"),
            ("kon'ya", "こんや"),
            ("nn", "ん"),
            ("n", "ん"),
            ("thi", "てぃ"),
            ("xya", "ゃ"),
            ("😀", "😀"),
        ] {
            assert_eq!(hiragana(raw, true), expected, "{raw}");
        }
        assert_eq!(hiragana("ky", false), "ky");
        assert_eq!(hiragana("n", false), "n");
    }
}
