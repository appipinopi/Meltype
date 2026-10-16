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
    kana_words: HashSet<String>,
    common_english: HashSet<String>,
    readable_english: HashSet<String>,
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
                kana_words: include_str!("../../../dictionaries/readings.txt")
                    .lines()
                    .filter(|l| !l.starts_with('#'))
                    .filter_map(|l| l.split_whitespace().next())
                    .map(str::to_owned)
                    .collect(),
                common_english: words(include_str!("../../../dictionaries/english.txt"))
                    .map(str::to_owned)
                    .collect(),
                readable_english: words(include_str!("../../../dictionaries/english-readable.txt"))
                    .map(str::to_owned)
                    .collect(),
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
            dict.english.extend(dict.readable_english.iter().cloned());
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

    /// Deterministic reverse lookup for selection reconversion. Ambiguous
    /// readings prefer the shortest reading, then lexical order.
    pub fn reading_for(&self, text: &str) -> Option<String> {
        self.candidates
            .iter()
            .filter(|(_, words)| words.iter().any(|word| word == text))
            .map(|(reading, _)| reading)
            .min_by(|a, b| {
                a.chars()
                    .count()
                    .cmp(&b.chars().count())
                    .then_with(|| a.cmp(b))
            })
            .cloned()
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
        let first = raw
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let english_sentence = raw.contains(' ')
            && [
                "i", "the", "please", "what", "how", "this", "my", "do", "let", "can", "make",
                "git", "npm", "good", "thank", "see", "run", "open", "meeting",
            ]
            .contains(&first.as_str());
        self.mixed_context(raw, final_input, english_sentence)
    }
    pub fn mixed_context(&self, raw: &str, final_input: bool, english_context: bool) -> String {
        let mut out = String::new();
        let mut start = 0;
        // Preserve URLs, email, file names, and agent commands as whole spans.
        for token in raw.split_inclusive(char::is_whitespace) {
            let word = token.trim_end_matches(char::is_whitespace);
            if word.contains("://")
                || word.bytes().any(|c| c.is_ascii_digit())
                    && word.bytes().any(|c| c.is_ascii_alphabetic())
                    && word.bytes().all(|c| c.is_ascii_alphanumeric())
                || word.contains('@')
                || word.starts_with(['/', '$'])
                || word.contains('\\')
                || word.contains('_')
                || word.split_once('.').is_some_and(|(left, right)| {
                    !left.is_empty()
                        && !right.is_empty()
                        && left.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
                        && right
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || c == b'.')
                })
            {
                out.push_str(token);
            } else {
                let mut run = String::new();
                for c in token.chars().chain(std::iter::once('\0')) {
                    if c.is_ascii_alphabetic() || c == '\'' {
                        run.push(c);
                    } else {
                        if !run.is_empty() {
                            if english_context && self.is_english(&run) {
                                out.push_str(&run);
                            } else {
                                out.push_str(&self.mixed_word(&run, final_input));
                            }
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
        if !raw.is_empty() && raw.bytes().all(|c| c.is_ascii_uppercase()) {
            return raw.to_owned();
        }
        if ["no", "ya", "repo"].contains(&lower.as_str()) {
            return romaji::hiragana(raw, final_input);
        }
        let suffix = if lower.ends_with("pedias") {
            6
        } else if lower.ends_with("pedia") {
            5
        } else {
            0
        };
        if suffix > 0 && (lower.len() == suffix || lower.len() >= suffix + 3) {
            return raw.to_owned();
        }
        if self.japanese.contains(&lower) {
            return romaji::hiragana(raw, final_input);
        }
        let reading = romaji::hiragana(raw, final_input);
        if !self.readable_english.contains(&lower) && self.kana_words.contains(&reading) {
            return reading;
        }
        if self.english.contains(&lower) {
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
                let piece_reading = romaji::hiragana(piece, true);
                let jp = self.japanese.contains(piece)
                    || self.kana_words.contains(&piece_reading)
                    || ["ha", "wa", "wo", "ga", "no", "ni", "de", "to", "mo", "he"]
                        .contains(&piece);
                let acronym =
                    piece.len() >= 2 && raw[i..end].bytes().all(|c| c.is_ascii_uppercase());
                let en = (piece.len() >= 3 && self.english.contains(piece) && !jp) || acronym;
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
    pub fn is_english(&self, raw: &str) -> bool {
        self.english.contains(&raw.to_ascii_lowercase())
    }
    pub fn is_common_english(&self, raw: &str) -> bool {
        self.common_english.contains(&raw.to_ascii_lowercase())
    }
    pub fn is_japanese(&self, raw: &str) -> bool {
        self.japanese.contains(&raw.to_ascii_lowercase())
            || self.kana_words.contains(&romaji::hiragana(raw, true))
    }
    pub fn english_prefix(&self, raw: &str) -> bool {
        let raw = raw.to_ascii_lowercase();
        self.english.iter().any(|w| w.starts_with(&raw))
    }
    pub fn japanese_prefix(&self, raw: &str) -> bool {
        let raw = raw.to_ascii_lowercase();
        self.japanese.iter().any(|w| w.starts_with(&raw))
    }
    pub fn predict_english(&self, raw: &str) -> Vec<String> {
        if raw.len() < 3 || !raw.bytes().all(|c| c.is_ascii_alphabetic()) {
            return vec![];
        }
        let lower = raw.to_ascii_lowercase();
        let mut words: Vec<_> = self
            .english
            .iter()
            .filter(|s| s.starts_with(&lower) && s.len() > lower.len())
            .collect();
        words.sort_by_key(|s| (!self.common_english.contains(*s), s.len(), *s));
        words
            .into_iter()
            .take(5)
            .map(|s| {
                if raw.bytes().all(|c| c.is_ascii_uppercase()) {
                    s.to_ascii_uppercase()
                } else if raw.as_bytes()[0].is_ascii_uppercase() {
                    let mut s = s.clone();
                    s[..1].make_ascii_uppercase();
                    s
                } else {
                    s.clone()
                }
            })
            .collect()
    }
}
