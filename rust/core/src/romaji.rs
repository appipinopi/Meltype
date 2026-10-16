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

#[derive(Debug, Clone)]
pub struct Token {
    pub spelling: String,
    pub kana: String,
}
#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub valid: bool,
    pub tokens: Vec<Token>,
    pub partial: String,
    pub strong_youon: usize,
    pub tsu: usize,
    pub sokuon: usize,
    pub long_vowels: usize,
}
impl Analysis {
    pub fn kana(&self) -> String {
        self.tokens.iter().map(|t| t.kana.as_str()).collect()
    }
    pub fn consumed(&self) -> usize {
        self.tokens.iter().map(|t| t.spelling.len()).sum()
    }
}

/// Upstream distinguishes strict language detection from composition spellings
/// such as l/x/v. Tokens retain source boundaries for conservative typo repair.
pub fn analyze(raw: &str, composition: bool) -> Analysis {
    static STRICT: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    let spellings = if composition {
        table()
    } else {
        STRICT.get_or_init(|| {
            include_str!("romaji-strict.tsv")
                .lines()
                .filter_map(|s| s.split_once('\t'))
                .collect()
        })
    };
    let mut result = Analysis::default();
    let mut i = 0;
    let bytes = raw.as_bytes();
    while i < raw.len() {
        let c = bytes[i];
        if !c.is_ascii_lowercase() {
            return result;
        }
        let next = bytes.get(i + 1).copied();
        let consonant = |b: u8| b.is_ascii_lowercase() && !b"aiueo".contains(&b);
        let mut matched = None;
        if c == b'n' && next.is_some_and(|b| b == b'n' || consonant(b) && b != b'y') {
            if i == 0 && !composition {
                return result;
            }
            let consumed = if next == Some(b'n')
                && (composition || bytes.get(i + 2).is_none_or(|b| !b"aiueoy".contains(b)))
            {
                2
            } else {
                1
            };
            matched = Some((consumed, "ん"));
        } else if consonant(c)
            && c != b'n'
            && (next == Some(c)
                || c == b't' && next == Some(b'c') && bytes.get(i + 2) == Some(&b'h'))
        {
            if i == 0 && !composition {
                return result;
            }
            result.sokuon += 1;
            matched = Some((1, "っ"));
        } else {
            for length in (1..=4.min(raw.len() - i)).rev() {
                if let Some(kana) = raw.get(i..i + length).and_then(|s| spellings.get(s)) {
                    matched = Some((length, *kana));
                    break;
                }
            }
        }
        if let Some((length, kana)) = matched {
            let piece = &raw[i..i + length];
            if [
                "きゃ", "きゅ", "きょ", "にゃ", "にゅ", "にょ", "ひゃ", "ひゅ", "ひょ", "みゃ",
                "みゅ", "みょ", "りゃ", "りゅ", "りょ", "ぎゃ", "ぎゅ", "ぎょ", "びゃ", "びゅ",
                "びょ", "ぴゃ", "ぴゅ", "ぴょ", "ちぇ", "てゃ", "てゅ", "てょ", "でゃ", "でゅ",
                "でょ", "にぃ", "にぇ", "とぁ", "とぃ", "とぇ", "とぅ", "どぁ", "どぃ", "どぇ",
                "どぅ",
            ]
            .contains(&kana)
            {
                result.strong_youon += 1;
            }
            if piece == "tsu" {
                result.tsu += 1;
            }
            if kana == "う"
                && result
                    .tokens
                    .last()
                    .is_some_and(|t| t.spelling.ends_with(['o', 'u']))
            {
                result.long_vowels += 1;
            }
            result.tokens.push(Token {
                spelling: piece.into(),
                kana: kana.into(),
            });
            i += length;
        } else {
            let rest = &raw[i..];
            if rest == "n" || rest == "tc" || spellings.keys().any(|s| s.starts_with(rest)) {
                result.valid = true;
                result.partial = rest.into();
            }
            return result;
        }
    }
    result.valid = true;
    result
}

pub fn spelling_variants(raw: &str, limit: usize) -> Vec<String> {
    let mut analysis = analyze(raw, false);
    if analysis.valid && analysis.partial == "n" {
        analysis = analyze(&format!("{raw}n"), false);
    }
    if !analysis.valid || !analysis.partial.is_empty() {
        return vec![raw.into()];
    }
    let mut spellings: HashMap<&str, Vec<&str>> = HashMap::new();
    for line in include_str!("romaji-strict.tsv").lines() {
        if let Some((spelling, kana)) = line.split_once('\t') {
            spellings.entry(kana).or_default().push(spelling);
        }
    }
    let mut results = vec![String::new()];
    for (index, token) in analysis.tokens.iter().enumerate() {
        let next = analysis.tokens.get(index + 1);
        let mut options: Vec<String> = Vec::new();
        match token.kana.as_str() {
            "ん" => {
                options.push("nn".into());
                if next.is_none_or(|t| {
                    t.spelling
                        .as_bytes()
                        .first()
                        .is_some_and(|b| !b"aiueoy".contains(b))
                }) {
                    options.push("n".into());
                }
            }
            "っ" => {
                if let Some(following) = next.and_then(|t| spellings.get(t.kana.as_str())) {
                    for spelling in following {
                        let first = spelling.as_bytes()[0];
                        if !b"aiueon".contains(&first)
                            && !options.contains(&(first as char).to_string())
                        {
                            options.push((first as char).to_string());
                        }
                    }
                }
                if options.is_empty() {
                    options.push(token.spelling.clone());
                }
            }
            _ => {
                options = spellings
                    .get(token.kana.as_str())
                    .map(|s| s.iter().map(|s| (*s).into()).collect())
                    .unwrap_or_else(|| vec![token.spelling.clone()]);
            }
        }
        results = results
            .into_iter()
            .flat_map(|prefix| {
                options
                    .iter()
                    .map(move |option| format!("{prefix}{option}"))
            })
            .take(limit.max(1))
            .collect();
    }
    if !results.iter().any(|s| s == raw) {
        results.insert(0, raw.into());
    }
    results
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
            _ => raw[raw.len() - rest.len()..]
                .chars()
                .next()
                .unwrap_or(first),
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
