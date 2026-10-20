// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Level-dependent English spans, following CompositionDetector's evidence rules.
use super::Dictionary;
use crate::{protection, romaji};

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
    fn side_character(c: char) -> Option<i32> {
        if c.is_ascii_alphanumeric() {
            Some(1)
        } else if c >= '\u{2000}' && !c.is_whitespace() {
            Some(-1)
        } else {
            None
        }
    }
    fn english_sentence(before: &str) -> bool {
        if !before.ends_with(' ') {
            return false;
        }
        let start = before
            .char_indices()
            .rev()
            .find(|(_, c)| !c.is_ascii() || matches!(c, '\r' | '\n'))
            .map_or(0, |(i, c)| i + c.len_utf8());
        let line_start = start == 0 || before[..start].ends_with(['\r', '\n']);
        let words: Vec<_> = before[start..]
            .split(' ')
            .filter(|w| !w.is_empty())
            .collect();
        if let [word] = words.as_slice() {
            let lower = word.trim_end_matches([',', '!', '.']).to_ascii_lowercase();
            if line_start
                && [
                    "hey", "hi", "hello", "oh", "wow", "yeah", "yes", "well", "so", "hmm", "ah",
                    "ooh", "oops", "thanks", "sorry", "please", "dear", "yay", "whoa", "nope",
                    "yep",
                ]
                .contains(&lower.as_str())
                || [
                    "oh", "wow", "yeah", "ooh", "oops", "whoa", "woah", "yay", "hmm", "hey",
                ]
                .contains(&lower.as_str())
            {
                return true;
            }
            if line_start
                && word.split_once('\'').is_some_and(|(a, b)| {
                    !a.is_empty()
                        && a.bytes().all(|c| c.is_ascii_alphabetic())
                        && (1..=2).contains(&b.len())
                        && b.bytes().all(|c| c.is_ascii_alphabetic())
                })
            {
                return true;
            }
        }
        words.len() >= 2
            && words.iter().all(|w| {
                w.bytes().any(|c| c.is_ascii_alphabetic())
                    && w.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b",.'-!?:;".contains(&c))
            })
    }
    pub(super) fn before_evidence(&self, before: &str) -> i32 {
        for c in before.chars().rev() {
            if let Some(score) = Self::side_character(c) {
                return if score == 1 && Self::english_sentence(before) {
                    2
                } else {
                    score
                };
            }
            if !c.is_ascii() && !c.is_whitespace() {
                break;
            }
        }
        0
    }
    pub(super) fn after_evidence(&self, after: &str) -> i32 {
        for c in after.chars() {
            if matches!(c, '\r' | '\n') {
                break;
            }
            if let Some(score) = Self::side_character(c) {
                return score;
            }
            if !c.is_ascii() && !c.is_whitespace() {
                break;
            }
        }
        0
    }
    pub fn composition(
        &self,
        raw: &str,
        final_input: bool,
        before: &str,
        after: &str,
        level: &str,
    ) -> String {
        let compute = || self.composition_uncached(raw, final_input, before, after, level);
        match romaji::probes::Cache::current() {
            Some(cache) => cache.composition(raw, final_input, before, after, level, compute),
            None => compute(),
        }
    }
    fn composition_uncached(
        &self,
        raw: &str,
        final_input: bool,
        before: &str,
        after: &str,
        level: &str,
    ) -> String {
        if !["Aggressive", "Conservative", "Manual"].contains(&level)
            && !crate::language_memory::relevant(raw)
        {
            // IsEnglishSpan requires two sides for a two-letter spelling
            // which also reads as Japanese. One preceding Latin word is not
            // an English sentence (k|ai after a direct-mode change).
            if raw.len() == 2 && raw.bytes().all(|b| b.is_ascii_lowercase()) {
                let analysis = romaji::analyze(raw, false);
                if analysis.valid
                    && ["", "n"].contains(&analysis.partial.as_str())
                    && self.before_evidence(before) + self.after_evidence(after) < 2
                    && !(before.ends_with(char::is_whitespace) && self.english_context(before))
                {
                    return self.mixed_context(raw, final_input, false);
                }
            }
            return self.mixed_context_evidence(
                raw,
                final_input,
                self.before_evidence(before),
                self.after_evidence(after),
                level,
            );
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
                    let history = format!("{before}{result}");
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
        start: bool,
        next: &str,
    ) -> bool {
        let analysis = romaji::analyze(&raw.to_ascii_lowercase(), false);
        let unreadable = !analysis.valid
            || !at_end
                && !analysis.partial.is_empty()
                && !["n", "nn"].contains(&analysis.partial.as_str());
        self.scored_english(
            raw,
            at_end,
            final_input,
            evidence.before,
            evidence.after,
            start,
            unreadable,
            next,
            evidence.level,
        )
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
        if self.allows_english(raw, true, final_input, evidence, true, "") {
            return raw.into();
        }
        if (self.is_english(raw) || self.is_japanese(raw))
            && crate::language_memory::get(&lower) != Some(false)
        {
            return romaji::hiragana(raw, final_input);
        }
        // Keep Japanese spans together while allowing confident English islands
        // (googleで検索, 日本語の後の unreadable words) in Conservative.
        let n = raw.len();
        let (bounds, pending_start) = romaji::unit_boundaries(
            &lower,
            |prefix| {
                (0..prefix.len().saturating_sub(2)).any(|start| {
                    let word = &prefix[start..];
                    self.common_english.contains(word)
                        || self.proper.contains(word)
                        || self.readable_english.contains(word)
                        || word.len() >= 4 && self.known_english(word)
                })
            },
            |word| self.known_english(word),
        );
        let mut costs = vec![f64::INFINITY; n + 1];
        let mut paths = vec![(0, false); n + 1];
        let after_scores: Vec<_> = (0..=n)
            .map(|end| if end == n { evidence.after } else { -1 })
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
                let english = bounds[start]
                    && bounds[end]
                    && (pending_start != Some(end) || end == n)
                    && self.allows_english(
                        piece,
                        end == n,
                        final_input,
                        Evidence {
                            before,
                            after: after_scores[end],
                            ..evidence
                        },
                        start == 0,
                        &raw[end..],
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
