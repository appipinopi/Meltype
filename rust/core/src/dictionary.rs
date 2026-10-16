// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use crate::romaji;
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

pub struct Dictionary {
    english: HashSet<String>,
    japanese: HashSet<String>,
    candidates: HashMap<String, Vec<String>>,
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .flat_map(|line| line.split('#').next().unwrap_or("").split_whitespace())
}

impl Dictionary {
    pub fn bundled() -> &'static Self {
        static DICT: OnceLock<Dictionary> = OnceLock::new();
        DICT.get_or_init(|| {
            let mut dict = Self {
                english: HashSet::new(),
                japanese: words(include_str!("../../../dictionaries/japanese.txt"))
                    .map(str::to_owned)
                    .collect(),
                candidates: HashMap::new(),
            };
            for source in [
                include_str!("../../../dictionaries/english.txt"),
                include_str!("../../../dictionaries/english-words.txt"),
                include_str!("../../../dictionaries/propernouns.txt"),
                include_str!("../../../dictionaries/brands.txt"),
            ] {
                dict.english
                    .extend(words(source).map(str::to_ascii_lowercase));
            }
            for source in [
                include_str!("../../../dictionaries/candidates.txt"),
                include_str!("../../../dictionaries/loanwords.txt"),
            ] {
                for line in source.lines() {
                    let fields: Vec<_> = line
                        .split('#')
                        .next()
                        .unwrap_or("")
                        .split_whitespace()
                        .collect();
                    if fields.len() > 1 {
                        dict.add(fields[0], fields[1..].iter().copied());
                    }
                }
            }
            for source in [
                include_str!("../../../dictionaries/phrases.txt"),
                include_str!("../../../dictionaries/emoji.txt"),
                include_str!("../../../dictionaries/emoji-cldr.txt"),
            ] {
                for line in source.lines().filter(|l| !l.starts_with('#')) {
                    let fields: Vec<_> = line.split('\t').collect();
                    if fields.len() > 1 {
                        dict.add(fields[0], fields[1..].iter().copied());
                    }
                }
            }
            dict
        })
    }

    fn add<'a>(&mut self, reading: &str, values: impl Iterator<Item = &'a str>) {
        let entry = self.candidates.entry(reading.to_owned()).or_default();
        for value in values.filter(|s| !s.is_empty()) {
            if !entry.iter().any(|v| v == value) {
                entry.push(value.to_owned());
            }
        }
    }

    pub fn candidates(&self, reading: &str) -> Vec<String> {
        let mut values = self.candidates.get(reading).cloned().unwrap_or_default();
        let converted = self.convert(reading);
        if converted != reading && !values.contains(&converted) {
            values.insert(0, converted);
        }
        for value in [reading.to_owned(), romaji::katakana(reading)] {
            if !values.contains(&value) {
                values.push(value);
            }
        }
        values
    }

    /// Dictionary fallback, used when no system kana-kanji converter is attached.
    /// Longest readings win; this is intentionally not a replacement for Mozc.
    pub fn convert(&self, reading: &str) -> String {
        let mut rest = reading;
        let mut out = String::new();
        while !rest.is_empty() {
            let longest = rest
                .char_indices()
                .map(|(i, c)| i + c.len_utf8())
                .rev()
                .find_map(|end| {
                    self.candidates
                        .get(&rest[..end])
                        .filter(|_| rest[..end].chars().count() >= 2)
                        .map(|v| (end, &v[0]))
                });
            if let Some((end, text)) = longest {
                out.push_str(text);
                rest = &rest[end..];
            } else {
                let c = rest.chars().next().unwrap();
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
        out
    }

    pub fn mixed(&self, raw: &str, final_input: bool) -> String {
        let mut out = String::new();
        let mut start = 0;
        // Preserve URLs, email, file names, and agent commands as whole spans.
        for token in raw.split_inclusive(char::is_whitespace) {
            let word = token.trim_end_matches(char::is_whitespace);
            if word.contains("://")
                || word.contains('@')
                || word.starts_with(['/', '$'])
                || word.contains('\\')
                || word.contains('_')
                || word.contains('.') && word.contains(char::is_alphanumeric)
            {
                out.push_str(token);
            } else {
                let mut run = String::new();
                for c in token.chars().chain(std::iter::once('\0')) {
                    if c.is_ascii_alphabetic() || c == '\'' {
                        run.push(c);
                    } else {
                        if !run.is_empty() {
                            out.push_str(&self.mixed_word(&run, final_input));
                            run.clear();
                        }
                        if c != '\0' {
                            out.push_str(&romaji::hiragana(&c.to_string(), true));
                        }
                    }
                }
            }
            start += token.len();
        }
        debug_assert_eq!(start, raw.len());
        out
    }

    fn mixed_word(&self, raw: &str, final_input: bool) -> String {
        let lower = raw.to_ascii_lowercase();
        if self.japanese.contains(&lower) {
            return romaji::hiragana(raw, final_input);
        }
        if self.english.contains(&lower)
            || raw.chars().filter(char::is_ascii_uppercase).count() >= 2
        {
            return raw.to_owned();
        }
        // Shortest-path segmentation over ASCII byte boundaries. Unknown syllables
        // remain kana; dictionary words can form English islands inside Japanese.
        let n = raw.len();
        let mut costs = vec![f64::INFINITY; n + 1];
        let mut paths = vec![(0, false); n + 1];
        costs[0] = 0.0;
        for i in 0..n {
            for end in i + 1..=n.min(i + 40) {
                let piece = &lower[i..end];
                let jp = self.japanese.contains(piece)
                    || ["ha", "wa", "wo", "ga", "no", "ni", "de", "to", "mo", "he"]
                        .contains(&piece);
                let en = piece.len() >= 3 && self.english.contains(piece) && !jp;
                let cost = if jp {
                    0.3
                } else if en {
                    1.0
                } else if end == i + 1 {
                    2.0
                } else {
                    continue;
                };
                if costs[i] + cost < costs[end] {
                    costs[end] = costs[i] + cost;
                    paths[end] = (i, en);
                }
            }
        }
        let mut pieces = Vec::new();
        let mut end = n;
        while end > 0 {
            let (begin, en) = paths[end];
            pieces.push((begin, end, en));
            end = begin;
        }
        pieces.reverse();
        // Merge adjacent Japanese spans before reading n / doubled consonants.
        let mut result = String::new();
        let mut japanese = String::new();
        for (begin, end, en) in pieces {
            if en {
                result.push_str(&romaji::hiragana(&japanese, true));
                japanese.clear();
                result.push_str(&raw[begin..end]);
            } else {
                japanese.push_str(&raw[begin..end]);
            }
        }
        result.push_str(&romaji::hiragana(&japanese, final_input));
        result
    }
}
