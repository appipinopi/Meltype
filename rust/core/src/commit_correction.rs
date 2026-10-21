// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Bounded, guarded correction of ambiguous words after the next word is known.
use crate::{dictionary::Dictionary, romaji, session::Preferences};
#[derive(Clone)]
pub(crate) struct Record {
    pub raw: String,
    pub text: String,
    pub english: bool,
    pub space: bool,
    pub after_english: bool,
}
pub(crate) fn replacement(
    records: &[Record],
    raw: &str,
    english: bool,
    before: &str,
    preferences: &Preferences,
) -> Option<(String, String)> {
    let previous = records.last()?;
    let dictionary = Dictionary::bundled();
    let (targets, replacement) = if !previous.english
        && english
        && (dictionary.is_english(raw) || !romaji::analyze(&raw.to_ascii_lowercase(), true).valid)
        && !dictionary.is_japanese(raw)
    {
        let mut start = records.len() - 1;
        while start > 0 && !records[start - 1].english && records[start - 1].space {
            start -= 1;
        }
        let targets = &records[start..];
        if targets.len() == 1
            && targets[0].raw.len() <= 2
            && targets[0].raw != "i"
            && raw.starts_with(|c: char| c.is_ascii_uppercase())
        {
            return None;
        }
        let keep_japanese = targets.iter().any(|r| {
            [
                "site", "sita", "suru", "shite", "shita", "shimasu", "shinai", "shiyou", "shiro",
                "sezu",
            ]
            .contains(&r.raw.as_str())
        });
        if keep_japanese && !targets.iter().any(|r| r.space) {
            return None;
        }
        let replacement = targets
            .iter()
            .map(|r| {
                let word = if keep_japanese {
                    r.text.trim_end()
                } else if r.raw == "i" {
                    "I"
                } else {
                    &r.raw
                };
                format!("{word}{}", if r.space { " " } else { "" })
            })
            .collect::<String>();
        (targets, replacement)
    } else if previous.english
        && !english
        && !dictionary.is_english(raw)
        && raw.bytes().any(|c| c.is_ascii_alphabetic())
        && (!previous.after_english || dictionary.is_japanese_origin(&previous.raw))
    {
        let replacement = romaji::hiragana(&previous.raw.to_ascii_lowercase(), true);
        if !crate::typo::Corrector::bundled().is_common_japanese(&previous.raw) {
            return None;
        }
        (&records[records.len() - 1..], replacement)
    } else {
        return None;
    };
    if targets
        .iter()
        .any(|r| preferences.english.get(&r.raw.to_ascii_lowercase()) == Some(&r.english))
    {
        return None;
    }
    let original: String = targets.iter().map(|r| r.text.as_str()).collect();
    if original == replacement || !before.ends_with(&original) {
        return None;
    }
    Some((original, replacement))
}
