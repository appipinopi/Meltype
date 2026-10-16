// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use crate::romaji;
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};
mod composition;

pub struct Dictionary {
    english: HashSet<String>,
    japanese: HashSet<String>,
    candidates: HashMap<String, Vec<String>>,
    emoji: HashMap<String, Vec<String>>,
    kana_words: HashSet<String>,
    common_english: HashSet<String>,
    readable_english: HashSet<String>,
    proper: HashSet<String>,
    japanese_origin: HashSet<String>,
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
                japanese_origin: {
                    let mut section = false;
                    let mut result = HashSet::new();
                    for line in include_str!("../../../dictionaries/english.txt").lines() {
                        if line.starts_with("# ---") {
                            section = line.contains("日本語由来");
                        } else if line.trim().is_empty() {
                            section = false;
                        } else if section && !line.starts_with('#') {
                            result.extend(words(line).map(str::to_owned));
                        }
                    }
                    result
                },
                proper: words(include_str!("../../../dictionaries/propernouns.txt"))
                    .chain(words(include_str!("../../../dictionaries/brands.txt")))
                    .filter(|word| word.is_ascii())
                    .map(str::to_ascii_lowercase)
                    .collect(),
                japanese: words(include_str!("../../../dictionaries/japanese.txt"))
                    .map(str::to_owned)
                    .collect(),
                candidates: HashMap::new(),
                emoji: HashMap::new(),
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
            for (source, emoji) in [
                (include_str!("../../../dictionaries/phrases.txt"), false),
                (include_str!("../../../dictionaries/emoji.txt"), true),
                (include_str!("../../../dictionaries/emoji-cldr.txt"), true),
            ] {
                for line in source.lines().filter(|l| !l.starts_with('#')) {
                    let fields: Vec<_> = line.split('\t').collect();
                    if fields.len() > 1 {
                        if emoji {
                            let entries = dict.emoji.entry(fields[0].to_owned()).or_default();
                            for word in &fields[1..] {
                                if !word.is_empty() && !entries.iter().any(|s| s == word) {
                                    entries.push((*word).to_owned());
                                }
                            }
                        } else {
                            dict.add(fields[0], fields[1..].iter().copied());
                        }
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
        let words = self.candidates.get(reading).cloned().unwrap_or_default();
        let mut values: Vec<_> = words
            .iter()
            .filter(|s| {
                s.chars()
                    .any(|c| crate::linguistics::kana(c) || crate::linguistics::kanji(c))
            })
            .cloned()
            .collect();
        let converted = self.convert(reading);
        if converted != reading && !values.contains(&converted) {
            values.insert(0, converted);
        }
        let katakana = romaji::katakana(reading);
        for value in [
            reading.to_owned(),
            katakana.clone(),
            crate::kana::halfwidth(&katakana),
        ] {
            if !values.contains(&value) {
                values.push(value);
            }
        }
        values.extend(crate::time_candidates::candidates(
            reading,
            chrono::Local::now().naive_local(),
        ));
        for word in words {
            if !values.contains(&word) {
                values.push(word);
            }
        }
        for word in self.emoji(reading) {
            if !values.contains(&word) {
                values.push(word);
            }
        }
        values
    }
    pub fn emoji(&self, reading: &str) -> Vec<String> {
        self.emoji
            .get(reading)
            .map(|words| words.iter().rev().cloned().collect())
            .unwrap_or_default()
    }

    /// Deterministic reverse lookup for selection reconversion. Ambiguous
    /// readings prefer the shortest reading, then lexical order.
    pub fn reading_for(&self, text: &str) -> Option<String> {
        self.candidates
            .iter()
            .chain(self.emoji.iter())
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
                        .and_then(|values| {
                            values
                                .iter()
                                .find(|s| {
                                    s.chars().any(|c| {
                                        crate::linguistics::kana(c) || crate::linguistics::kanji(c)
                                    })
                                })
                                .map(|s| (end, s))
                        })
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
                    && !self.number_with_japanese_unit(word)
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
                            if english_context && (self.is_english(&run) || !self.is_japanese(&run))
                            {
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

    pub fn english_context(&self, before: &str) -> bool {
        let sentence = before
            .rsplit(['。', '！', '？', '\n', '\r'])
            .next()
            .unwrap_or("");
        if sentence
            .chars()
            .any(|c| crate::linguistics::kana(c) || crate::linguistics::kanji(c))
        {
            return false;
        }
        let words: Vec<_> = sentence
            .split_whitespace()
            .map(|s| s.trim_matches(|c: char| c.is_ascii_punctuation()))
            .filter(|s| !s.is_empty())
            .collect();
        let Some(last) = words.last() else {
            return false;
        };
        if !last.bytes().all(|c| c.is_ascii_alphabetic()) {
            return false;
        }
        if self.proper.contains(&last.to_ascii_lowercase()) {
            return words[..words.len() - 1].iter().any(|word| {
                [
                    "i", "we", "you", "they", "the", "this", "my", "is", "are", "a", "an",
                    "please", "use", "have", "want", "like",
                ]
                .contains(&word.to_ascii_lowercase().as_str())
            });
        }
        true
    }
    fn number_with_japanese_unit(&self, word: &str) -> bool {
        let Some(start) = word.find(|c: char| c.is_ascii_digit()) else {
            return false;
        };
        let suffix = word[start..].trim_start_matches(|c: char| c.is_ascii_digit());
        [
            "ji", "fun", "pun", "gatsu", "nichi", "nen", "jikan", "en", "nin", "sai", "mai", "hon",
            "bon", "ko", "kai", "kakeru", "waru", "tasu", "hiku", "purasu", "mainasu", "iko-ru",
        ]
        .iter()
        .any(|unit| suffix.starts_with(unit))
    }
    fn mixed_word(&self, raw: &str, final_input: bool) -> String {
        let lower = raw.to_ascii_lowercase();
        if !raw.is_empty() && raw.bytes().all(|c| c.is_ascii_uppercase()) {
            return raw.to_owned();
        }
        if ["no", "ya", "repo", "to"].contains(&lower.as_str()) {
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
        // Preserve case-sensitive product names while separating an attached particle.
        for particle in [
            "no", "ga", "wo", "ni", "de", "to", "ha", "mo", "wa", "he", "ya",
        ] {
            if let Some(prefix) = raw.strip_suffix(particle) {
                if prefix.len() >= 3
                    && prefix.bytes().skip(1).any(|b| b.is_ascii_uppercase())
                    && prefix.bytes().any(|b| b.is_ascii_lowercase())
                {
                    return format!("{}{}", prefix, romaji::hiragana(particle, true));
                }
            }
        }
        // An English word on each side supplies evidence for a Japanese particle,
        // even when the same letters also form a valid Japanese syllable sequence.
        for index in 2..raw.len() {
            for particle in [
                "no", "ga", "wo", "ni", "de", "to", "ha", "mo", "wa", "he", "ya",
            ] {
                if let Some(right) = lower[index..].strip_prefix(particle) {
                    if right.len() >= 2
                        && self.english_atom(&lower[..index])
                        && self.english_atom(right)
                        && (!self.is_japanese(&lower[..index])
                            || self.proper.contains(&lower[..index]))
                        && (!self.is_japanese(right) || self.proper.contains(right))
                    {
                        return format!(
                            "{}{}{}",
                            &raw[..index],
                            romaji::hiragana(particle, true),
                            &raw[index + particle.len()..]
                        );
                    }
                }
            }
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
                    || [
                        "ha", "wa", "wo", "ga", "no", "ni", "de", "to", "mo", "he", "ya",
                    ]
                    .contains(&piece);
                let acronym =
                    piece.len() >= 2 && raw[i..end].bytes().all(|c| c.is_ascii_uppercase());
                let suffix_ok = end == n
                    || [
                        "no", "ga", "wo", "ni", "de", "to", "ha", "mo", "wa", "he", "ya", "sa",
                        "shi", "si", "su",
                    ]
                    .iter()
                    .any(|p| lower[end..].starts_with(p))
                    || self.english_atom(&lower[end..]);
                let en =
                    ((piece.len() >= 3 && self.english_atom(piece) && !jp) || acronym) && suffix_ok;
                let cost = if jp {
                    0.3
                } else if en {
                    let absorbs_particle = i > 0
                        && [
                            "no", "ga", "wo", "ni", "de", "to", "ha", "mo", "wa", "he", "ya",
                        ]
                        .iter()
                        .any(|p| {
                            piece
                                .strip_prefix(p)
                                .is_some_and(|rest| rest.len() >= 3 && self.english_atom(rest))
                        });
                    if absorbs_particle {
                        1.5
                    } else if !self.english.contains(piece) {
                        1.8
                    } else {
                        1.0
                    }
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
    fn english_atom(&self, word: &str) -> bool {
        if self.english.contains(word) {
            return true;
        }
        if word.len() >= 3
            && word
                .bytes()
                .all(|b| b.is_ascii_lowercase() && !b"aiueo".contains(&b))
            && !romaji::analyze(word, true).valid
        {
            return true;
        }
        if !word.is_ascii() {
            return false;
        }
        let particles = [
            "no", "ga", "wo", "ni", "de", "to", "ha", "mo", "wa", "he", "ya",
        ];
        (2..word.len().saturating_sub(1)).any(|i| {
            self.english.contains(&word[..i])
                && self.english.contains(&word[i..])
                && (i >= 3
                    || word[..i].bytes().all(|b| b"aiueo".contains(&b))
                    || word[..i].bytes().all(|b| !b"aiueo".contains(&b)))
                && (word.len() - i >= 3
                    || word[i..].bytes().all(|b| b"aiueo".contains(&b))
                    || word[i..].bytes().all(|b| !b"aiueo".contains(&b)))
                && !particles.contains(&&word[..i])
                && !particles.contains(&&word[i..])
                && (!self.is_japanese(&word[..i])
                    || i == 2 && self.common_english.contains(&word[..i]))
                && (!self.is_japanese(&word[i..])
                    || word.len() - i == 2 && self.common_english.contains(&word[i..]))
                && ![
                    "no", "ga", "wo", "ni", "de", "to", "ha", "mo", "wa", "he", "ya",
                ]
                .iter()
                .any(|p| {
                    word[i..]
                        .strip_prefix(p)
                        .is_some_and(|tail| tail.len() >= 3 && self.english.contains(tail))
                })
        })
    }
    pub fn is_english(&self, raw: &str) -> bool {
        self.english.contains(&raw.to_ascii_lowercase())
    }
    pub fn is_japanese_origin(&self, raw: &str) -> bool {
        self.japanese_origin.contains(&raw.to_ascii_lowercase())
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
