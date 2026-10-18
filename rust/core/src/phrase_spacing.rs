// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 hrmcngs, appipinopi
//! The upstream EnglishPhraseSpacing token rules and embedded word boundaries.
use std::{collections::HashMap, sync::OnceLock};
fn boundaries() -> &'static HashMap<String, Vec<usize>> {
    static BOUNDARIES: OnceLock<HashMap<String, Vec<usize>>> = OnceLock::new();
    BOUNDARIES.get_or_init(|| {
        include_str!("../../../dictionaries/english-phrases.txt")
            .lines()
            .filter_map(|line| {
                if line.starts_with('#') {
                    return None;
                }
                let (key, phrase) = line.trim_end_matches('\r').split_once('\t')?;
                if phrase.contains('\t') {
                    return None;
                }
                let words: Vec<_> = phrase.split(' ').filter(|part| !part.is_empty()).collect();
                (words.concat() == key).then(|| {
                    (
                        key.to_ascii_lowercase(),
                        words.iter().map(|word| word.len()).collect(),
                    )
                })
            })
            .collect()
    })
}
fn token(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_-+#.'@/:".contains(c)
}
pub fn format(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap();
        if !token(ch) {
            result.push(ch);
            at += ch.len_utf8();
            continue;
        }
        let end = text[at..]
            .find(|c: char| !token(c))
            .map_or(text.len(), |n| at + n);
        let word = &text[at..end];
        if let Some(lengths) = boundaries().get(&word.to_ascii_lowercase()) {
            let mut offset = 0;
            for &length in lengths {
                if offset != 0 {
                    result.push(' ');
                }
                result.push_str(&word[offset..offset + length]);
                offset += length;
            }
        } else {
            result.push_str(word);
        }
        at = end;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_case_preserving_boundaries_and_structural_tokens() {
        for (input, expected) in [
            ("seeyouagain", "see you again"),
            ("SeeYouAgain!", "See You Again!"),
            ("日本語THANKYOUまたね", "日本語THANK YOUまたね"),
            (
                "@seeyouagain https://thankyou seeyouagain.",
                "@seeyouagain https://thankyou seeyouagain.",
            ),
        ] {
            assert_eq!(format(input), expected);
        }
    }
}
