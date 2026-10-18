// SPDX-License-Identifier: GPL-3.0-or-later
//! CompositionText's retained sound units. A removed following vowel must not
//! turn an already established small tsu back into a pending consonant.
use crate::romaji::{self, Token};

#[derive(Default)]
pub(crate) struct Units {
    raw: String,
    units: Vec<Token>,
    pending: String,
    english_edit: bool,
    japanese_edit: bool,
}
impl Units {
    pub fn capture(raw: &str, display: &str) -> Self {
        let mut value = Self::default();
        let mut cursor = 0;
        // English islands end their units before a Japanese suffix (github|ya).
        for english in display
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|s| !s.is_empty())
        {
            let offset = if display.ends_with(english) && raw.ends_with(english) {
                raw[cursor..].rfind(english)
            } else {
                raw[cursor..].find(english)
            };
            if let Some(offset) = offset {
                let end = cursor + offset + english.len();
                value.push(&raw[cursor..end]);
                value.finish_pending();
                cursor = end;
            }
        }
        value.push(&raw[cursor..]);
        value.japanese_edit = !display.chars().any(|c| c.is_ascii_alphabetic())
            && display
                .chars()
                .any(|c| crate::linguistics::kana(c) || crate::linguistics::kanji(c));
        value
    }
    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn kana(&self, final_input: bool) -> String {
        self.kana_range(0, self.raw.len(), final_input)
    }
    pub fn push(&mut self, text: &str) {
        self.english_edit = false;
        self.japanese_edit = false;
        // CompositionText.Append returns unreadable letters to Pending so a
        // following vowel/y can form a syllable after an edit (goog + ya).
        if text.len() == 1 && text.as_bytes()[0].is_ascii_alphabetic() {
            while let Some(last) = self.units.last() {
                if last.spelling.len() != 1
                    || last.kana != last.spelling
                    || !last.spelling.as_bytes()[0].is_ascii_alphabetic()
                {
                    break;
                }
                let letter = last.spelling.as_bytes()[0].to_ascii_lowercase();
                let known = crate::dictionary::Dictionary::bundled().known_english(&self.raw);
                if known
                    && (matches!(letter, b'l' | b'x')
                        || letter == b't' && text.eq_ignore_ascii_case("s")
                        || text.eq_ignore_ascii_case("y"))
                {
                    break;
                }
                if last.spelling.as_bytes()[0].is_ascii_uppercase()
                    && self.units.len() >= 2
                    && self.units[self.units.len() - 2]
                        .spelling
                        .as_bytes()
                        .last()
                        .is_some_and(u8::is_ascii_uppercase)
                {
                    break;
                }
                let last = self.units.pop().expect("last unit");
                self.pending.insert_str(0, &last.spelling);
            }
        }
        self.raw.push_str(text);
        self.pending.push_str(text);
        self.normalize();
    }
    fn normalize(&mut self) {
        while !self.pending.is_empty() {
            let analysis = romaji::analyze(&self.pending.to_ascii_lowercase(), true);
            let mut offset = 0;
            for token in &analysis.tokens {
                let end = offset + token.spelling.len();
                self.units.push(Token {
                    spelling: self.pending[offset..end].into(),
                    kana: token.kana.clone(),
                });
                offset = end;
            }
            self.pending.drain(..offset);
            if analysis.valid {
                break;
            }
            let first = self.pending.chars().next().expect("unreadable input");
            self.units.push(Token {
                spelling: first.to_string(),
                kana: first.to_string(),
            });
            self.pending.drain(..first.len_utf8());
        }
    }
    fn finish_pending(&mut self) {
        for c in self.pending.chars() {
            self.units.push(Token {
                spelling: c.to_string(),
                kana: c.to_string(),
            });
        }
        self.pending.clear();
    }
    pub fn remove_last(&mut self, raw_character: bool, english_word: bool) {
        self.english_edit = english_word;
        if !self.pending.is_empty() {
            self.pending.pop();
        } else if let Some(last) = self.units.pop() {
            if raw_character {
                let mut remaining = last.spelling;
                remaining.pop();
                self.pending = remaining;
                self.normalize();
            }
        }
        self.raw = self
            .units
            .iter()
            .map(|u| u.spelling.as_str())
            .collect::<String>()
            + &self.pending;
    }
    fn kana_range(&self, start: usize, end: usize, final_input: bool) -> String {
        let mut result = String::new();
        let mut cursor = 0;
        for unit in &self.units {
            let next = cursor + unit.spelling.len();
            if cursor >= start && next <= end {
                result.push_str(&unit.kana);
            } else if cursor < end && next > start {
                return romaji::hiragana(&self.raw[start..end], final_input);
            }
            cursor = next;
        }
        if cursor < end {
            result.push_str(&romaji::hiragana(
                &self.raw[start.max(cursor)..end],
                final_input,
            ));
        }
        result
    }
    pub fn render(&self, detected: &str, final_input: bool) -> String {
        if self.units.is_empty() {
            return romaji::hiragana(&self.pending, final_input);
        }
        if self.japanese_edit {
            return self.kana_range(0, self.raw.len(), final_input);
        }
        if self.english_edit
            && self.raw.bytes().all(|c| c.is_ascii_alphabetic())
            && !romaji::analyze(&self.raw.to_ascii_lowercase(), false).valid
            && !self
                .units
                .iter()
                .any(|u| u.kana == "っ" && u.spelling.len() == 1)
        {
            return self.raw.clone();
        }
        let mut result = String::new();
        let mut cursor = 0;
        for english in detected
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|s| !s.is_empty())
        {
            let Some(offset) = self.raw[cursor..].find(english) else {
                return detected.into();
            };
            let start = cursor + offset;
            let end = start + english.len();
            {
                let mut boundary = 0;
                let mut boundaries = vec![0];
                for unit in &self.units {
                    boundary += unit.spelling.len();
                    boundaries.push(boundary);
                }
                boundaries.push(self.raw.len());
                if !boundaries.contains(&start) || !boundaries.contains(&end) {
                    return if crate::dictionary::Dictionary::bundled()
                        .unknown_english_word(&self.raw.to_ascii_lowercase())
                    {
                        self.raw.clone()
                    } else {
                        self.kana(final_input)
                    };
                }
            }
            result.push_str(&self.kana_range(cursor, start, true));
            // A retained sokuon is Japanese even after its next unit was erased.
            let mut position = 0;
            let retained = self.units.iter().any(|u| {
                let at = position;
                position += u.spelling.len();
                at >= start && position <= end && u.kana == "っ"
            });
            if retained
                && (english.len() == 1
                    || !crate::dictionary::Dictionary::bundled().is_english(english)
                        && !crate::dictionary::Dictionary::bundled()
                            .unknown_english_word(&english.to_ascii_lowercase()))
            {
                result.push_str(&self.kana_range(start, end, final_input));
            } else {
                result.push_str(english);
            }
            cursor = end;
        }
        result.push_str(&self.kana_range(cursor, self.raw.len(), final_input));
        result
    }
}
