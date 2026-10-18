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
            let _builtins = romaji::Scope::new(None);
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
        // CompositionDetector.IsEnglishSpan: a growing proper-name prefix at
        // the start of input is English unless it is also a Japanese prefix.
        if !final_input
            && lower.len() >= 4
            && (self.proper.iter().any(|word| word.starts_with(&lower))
                || !romaji::analyze(&lower, false).valid
                    && self
                        .common_english
                        .iter()
                        .any(|word| word.starts_with(&lower)))
            && !self.japanese_prefix(&lower)
            && (romaji::analyze(&lower, false).valid || {
                let fragment = romaji::analyze(&lower, true);
                !fragment.valid || !fragment.partial.is_empty()
            })
        {
            return raw.to_owned();
        }
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
            let fragment = romaji::analyze(&lower, true);
            let strict = romaji::analyze(&lower, false);
            if !self.readable_english.contains(&lower)
                && !self.proper.contains(&lower)
                && fragment.valid
                && (strict.valid && fragment.partial == "n"
                    || !final_input && !fragment.partial.is_empty() && self.japanese_prefix(&lower))
            {
                return romaji::hiragana(raw, final_input);
            }
            if lower.len() < 6
                && fragment.valid
                && fragment.partial.is_empty()
                && !romaji::analyze(&lower, false).valid
                && !self.common_english.contains(&lower)
                && !self.proper.contains(&lower)
                && !self.readable_english.contains(&lower)
            {
                return romaji::hiragana(raw, final_input);
            }
            if lower.len() <= 3
                && fragment.valid
                && fragment.partial.is_empty()
                && !self.readable_english.contains(&lower)
            {
                return romaji::hiragana(raw, final_input);
            }
            return raw.to_owned();
        }
        if self.spell_word(&lower) {
            let analysis = romaji::analyze(&lower, false);
            let fragment = romaji::analyze(&lower, true);
            let small_kana =
                !analysis.valid && fragment.valid && fragment.partial.is_empty() && lower.len() < 6;
            if !small_kana
                && (!fragment.valid
                    || !analysis.partial.is_empty()
                        && analysis.partial != "n"
                        && analysis.partial != "nn")
            {
                return raw.to_owned();
            }
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
        let boundaries = romaji::unit_boundaries(
            &lower,
            |prefix| {
                (0..prefix.len().saturating_sub(2)).any(|start| {
                    let word = &prefix[start..];
                    self.common_english.contains(word)
                        || self.proper.contains(word)
                        || self.readable_english.contains(word)
                        || word.len() >= 4 && (self.is_english(word) || self.spell_word(word))
                })
            },
            |prefix| self.known_english(prefix),
        );
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
                let english_piece = boundaries[i]
                    && boundaries[end]
                    && ((piece.len() >= 3
                        && !jp
                        && (self.english.contains(piece) || self.spell_word(piece)))
                        || acronym);
                // IsEnglishSpan keeps a short composition-only spelling in
                // Japanese when its next sound continues it (hosu|thin|gu).
                let japanese_continuation = english_piece
                    && i > 0
                    && !paths[i].1
                    && end < n
                    && piece.len() <= 4
                    && !romaji::analyze(piece, false).valid
                    && {
                        let fragment = romaji::analyze(piece, true);
                        fragment.valid
                            && ["", "n"].contains(&fragment.partial.as_str())
                            && (end + 1..=n)
                                .find(|next| boundaries[*next])
                                .is_some_and(|next| romaji::analyze(&lower[i..next], true).valid)
                    };
                let strict = romaji::analyze(piece, false);
                let partial_japanese = english_piece
                    && end == n
                    && strict.valid
                    && !strict.partial.is_empty()
                    && !self.proper.contains(piece)
                    && !self.readable_english.contains(piece)
                    && (strict.partial == "n"
                        || !final_input && self.japanese_prefix(piece)
                        || i > 0 && !paths[i].1 && !self.common_english.contains(piece));
                let en = english_piece && !japanese_continuation && !partial_japanese;
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
        let first_english = pieces.first().is_some_and(|(_, _, english)| *english);
        if !first_english && !self.known_english(&lower) {
            for end in 3..lower.len() {
                let stem = &lower[..end];
                if romaji::analyze(stem, true).valid || end >= 5 && self.known_english(stem) {
                    continue;
                }
                let read = romaji::analyze(stem, false).consumed();
                if read >= 2 {
                    continue;
                }
                let after_invalid = romaji::analyze(&stem[read + 1..], false);
                if after_invalid.valid && after_invalid.partial.is_empty() {
                    continue;
                }
                let rest = &lower[end..];
                let analysis = romaji::analyze(rest, false);
                if [
                    "kara", "made", "yori", "ga", "wo", "ni", "de", "no", "to", "mo", "ha", "wa",
                    "ya",
                ]
                .iter()
                .any(|p| rest.starts_with(p))
                    && analysis.valid
                    && ["", "n"].contains(&analysis.partial.as_str())
                {
                    return format!(
                        "{}{}",
                        &raw[..end],
                        romaji::hiragana(&raw[end..], final_input)
                    );
                }
            }
        }
        // CompositionDetector.Segment keeps an unknown word whole when its
        // first unreadable spelling is within the initial three characters.
        if !first_english && self.unknown_english_word(&lower) {
            let particle_then_word = [
                "kara", "made", "yori", "ga", "wo", "ni", "de", "no", "to", "mo", "ha", "wa", "ya",
            ]
            .iter()
            .find_map(|p| lower.strip_prefix(p))
            .is_some_and(|rest| rest.len() >= 3 && self.known_english(rest));
            if self.known_english(&lower) || !particle_then_word {
                return raw.to_owned();
            }
        }
        // HasListedEnglishPrefixSplit: a short listed head followed only by
        // unrecognised Latin letters is an unknown English word, rather than
        // an English prefix with an accidental Japanese reading.
        if let Some(&(0, head_end, true)) = pieces.first() {
            if head_end <= 3
                && head_end < n
                && (self.common_english.contains(&lower[..head_end])
                    || self.proper.contains(&lower[..head_end]))
                && pieces.iter().skip(1).all(|(_, _, en)| !en)
                && ![
                    "kara", "made", "yori", "ga", "wo", "ni", "de", "no", "to", "mo", "ha", "wa",
                    "ya",
                ]
                .iter()
                .any(|p| lower[head_end..].starts_with(p))
                && !(head_end + 1..=n).any(|end| self.known_english(&lower[..end]))
            {
                return raw.to_owned();
            }
        }
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
    pub(crate) fn unknown_english_word(&self, lower: &str) -> bool {
        if lower.len() < 4 || !lower.bytes().all(|b| b.is_ascii_alphabetic()) {
            return false;
        }
        let analysis = romaji::analyze(lower, false);
        if analysis.valid || romaji::analyze(lower, true).valid {
            return false;
        }
        let invalid = analysis.consumed();
        if invalid >= 3 {
            return false;
        }
        let rest = &lower[invalid + 1..];
        if rest.len() >= 3 && romaji::analyze(rest, true).valid {
            return false;
        }
        !crate::detection::japanese_typo(lower)
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
    pub fn spell_word(&self, raw: &str) -> bool {
        crate::spelling::is_word(raw)
    }
    pub fn known_english(&self, raw: &str) -> bool {
        self.is_english(raw) || self.spell_word(raw)
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
