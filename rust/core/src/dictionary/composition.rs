// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Level-dependent English spans, following CompositionDetector's evidence rules.
use super::Dictionary;
use crate::{linguistics, protection, romaji};

#[derive(Clone, Copy)]
struct Evidence<'a> {
    level: &'a str,
    before: i32,
    after: i32,
}
const PARTICLES: &[&str] = &[
    "no", "ga", "wo", "ni", "de", "to", "ha", "mo", "wa", "he", "ya",
];

impl Dictionary {
    fn side(&self, word: &str) -> i32 {
        if word
            .chars()
            .any(|c| linguistics::kana(c) || linguistics::kanji(c))
        {
            return -1;
        }
        if !word.is_empty()
            && word.bytes().all(|c| c.is_ascii_alphabetic())
            && (self.is_english(word)
                || word.starts_with(|c: char| c.is_ascii_uppercase())
                || word.len() >= 3 && !romaji::analyze(&word.to_ascii_lowercase(), true).valid)
        {
            1
        } else {
            0
        }
    }
    fn before_evidence(&self, before: &str) -> i32 {
        let sentence = before
            .rsplit(['。', '！', '？', '.', '!', '?', '\n', '\r'])
            .next()
            .unwrap_or_default();
        let mut words = sentence.split_whitespace().rev();
        let Some(last) = words.next() else {
            return 0;
        };
        let score = self.side(last.trim_matches(|c: char| c.is_ascii_punctuation()));
        if score == 1
            && before.ends_with(char::is_whitespace)
            && words.next().is_some_and(|w| self.side(w) == 1)
        {
            2
        } else {
            score
        }
    }
    fn after_evidence(&self, after: &str) -> i32 {
        self.side(
            after
                .trim_start()
                .split([' ', '\t', '\n', '\r', '。', '.', '!', '?'])
                .next()
                .unwrap_or_default(),
        )
    }
    pub(crate) fn composition(
        &self,
        raw: &str,
        final_input: bool,
        before: &str,
        after: &str,
        level: &str,
    ) -> String {
        if !["Aggressive", "Conservative", "Manual"].contains(&level) {
            return self.mixed_context(raw, final_input, self.english_context(before));
        }
        if raw.starts_with('$') && raw.bytes().all(|c| c.is_ascii_graphic()) {
            return raw.into();
        }
        protection::transform(raw, |part| {
            let mut result = String::new();
            let mut cursor = 0;
            while cursor < part.len() {
                let ch = part[cursor..].chars().next().unwrap();
                if ch.is_ascii_alphabetic() {
                    let end = part[cursor..]
                        .find(|c: char| !c.is_ascii_alphabetic())
                        .map_or(part.len(), |n| cursor + n);
                    let history = format!("{before}{}", &part[..cursor]);
                    let next = if end == part.len() {
                        after
                    } else {
                        &part[end..]
                    };
                    result.push_str(&self.composition_word(
                        &part[cursor..end],
                        final_input && end == part.len(),
                        Evidence {
                            level,
                            before: self.before_evidence(&history),
                            after: self.after_evidence(next),
                        },
                    ));
                    cursor = end;
                } else {
                    result.push_str(&romaji::hiragana(&ch.to_string(), true));
                    cursor += ch.len_utf8();
                }
            }
            result
        })
    }
    fn allows_english(
        &self,
        raw: &str,
        at_end: bool,
        final_input: bool,
        evidence: Evidence<'_>,
    ) -> bool {
        if raw.starts_with(|c: char| c.is_ascii_uppercase()) {
            return true;
        }
        if evidence.level == "Manual" {
            return false;
        }
        let lower = raw.to_ascii_lowercase();
        if lower.len() >= 3 && self.readable_english.contains(&lower) {
            return true;
        }
        // Compound English words can be joined by the span search below.
        // Avoid repeatedly re-segmenting unknown fragments inside that search.
        let abbreviation = lower.len() >= 3 && lower.bytes().all(|c| !b"aiueo".contains(&c));
        if !self.english.contains(&lower) && !abbreviation {
            return false;
        }
        let strict = romaji::analyze(&lower, false);
        let fragment = romaji::analyze(&lower, true);
        if !fragment.valid {
            return lower.len() >= 3 || self.common_english.contains(&lower);
        }
        if strict.sokuon > 0 && lower.len() >= 4 && !self.is_japanese(&lower) {
            return true;
        }
        if at_end
            && lower.len() >= 3
            && !fragment.partial.is_empty()
            && fragment.partial != "n"
            && (final_input || !self.japanese.iter().any(|w| w.starts_with(&lower)))
        {
            return true;
        }
        if evidence.level == "Conservative" && lower.len() < 3 && evidence.before < 2 {
            return false;
        }
        // A readable proper name requires context in Conservative, too.
        // Aggressive otherwise uses the existing dictionary rendering rules.
        let needed = if evidence.level == "Conservative" {
            2
        } else {
            1
        };
        let before = if evidence.after < 0 {
            evidence.before.min(1)
        } else {
            evidence.before
        };
        before + evidence.after >= needed
    }
    fn composition_word(&self, raw: &str, final_input: bool, evidence: Evidence<'_>) -> String {
        let lower = raw.to_ascii_lowercase();
        // Preserve an explicitly capitalized word, separating an attached particle.
        if raw.starts_with(|c: char| c.is_ascii_uppercase()) {
            for end in (2..raw.len()).rev() {
                let tail = &lower[end..];
                if self.is_english(&raw[..end])
                    && PARTICLES.iter().any(|p| tail.starts_with(p))
                    && romaji::analyze(tail, true).valid
                {
                    return format!(
                        "{}{}",
                        &raw[..end],
                        romaji::hiragana(&raw[end..], final_input)
                    );
                }
            }
            return raw.into();
        }
        if evidence.level == "Manual" {
            if let Some(index) = raw.find(|c: char| c.is_ascii_uppercase()) {
                return format!(
                    "{}{}",
                    romaji::hiragana(&raw[..index], true),
                    self.composition_word(&raw[index..], final_input, evidence)
                );
            }
            return romaji::hiragana(raw, final_input);
        }
        if self.allows_english(raw, true, final_input, evidence) {
            return raw.into();
        }
        if evidence.level == "Aggressive" {
            if !final_input
                && lower.len() >= 3
                && evidence.before >= 1
                && self.english_prefix(&lower)
                && !self.is_japanese(raw)
            {
                return raw.into();
            }
            return self.mixed_context(raw, final_input, evidence.before + evidence.after >= 1);
        }
        if self.is_english(raw) || self.is_japanese(raw) {
            return romaji::hiragana(raw, final_input);
        }
        // Keep Japanese spans together while allowing confident English islands
        // (googleで検索, 日本語の後の unreadable words) in Conservative.
        let n = raw.len();
        let mut costs = vec![f64::INFINITY; n + 1];
        let mut paths = vec![(0, false); n + 1];
        let after_scores: Vec<_> = (0..=n)
            .map(|end| {
                if end == n {
                    evidence.after
                } else if PARTICLES.iter().any(|p| lower[end..].starts_with(p)) {
                    -1
                } else {
                    self.side(&raw[end..])
                }
            })
            .collect();
        costs[0] = 0.;
        for start in 0..n {
            for end in start + 1..=n.min(start + 40) {
                let piece = &raw[start..end];
                let before = if start == 0 {
                    evidence.before
                } else if paths[start].1 {
                    1
                } else {
                    -1
                };
                let english = self.allows_english(
                    piece,
                    end == n,
                    final_input,
                    Evidence {
                        before,
                        after: after_scores[end],
                        ..evidence
                    },
                );
                let cost = if english {
                    1.
                } else if self.is_japanese(piece) || PARTICLES.contains(&piece) {
                    0.3
                } else if end == start + 1 {
                    2.
                } else {
                    continue;
                };
                if costs[start] + cost < costs[end] {
                    costs[end] = costs[start] + cost;
                    paths[end] = (start, english);
                }
            }
        }
        let mut pieces = vec![];
        let mut end = n;
        while end > 0 {
            let (start, english) = paths[end];
            pieces.push((start, end, english));
            end = start;
        }
        let mut out = String::new();
        let mut japanese = String::new();
        for (start, end, english) in pieces.into_iter().rev() {
            if english {
                out.push_str(&romaji::hiragana(&japanese, true));
                japanese.clear();
                out.push_str(&raw[start..end]);
            } else {
                japanese.push_str(&raw[start..end]);
            }
        }
        out.push_str(&romaji::hiragana(&japanese, final_input));
        out
    }
}
