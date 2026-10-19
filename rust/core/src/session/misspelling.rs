// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use crate::{composition_units::Units, romaji::Token};

pub(super) struct Edit {
    units: Units,
    start: usize,
    end: usize,
    right: String,
}
impl Session {
    pub(super) fn composition_misspelling(&self) -> Option<Edit> {
        if self.function_key != 0
            || self.explicit.is_some()
            || self.raw.is_empty()
            || self.reconversion_origin.is_some()
        {
            return None;
        }
        let mut display = if self.uses_kana() {
            self.kana_text.clone()
        } else {
            self.detect_composition(&self.raw, false)
        };
        // A lone trailing pending n belongs to the Japanese run, not to an
        // English island. Keep it pending until the dictionary includes ん.
        if display.ends_with('n')
            && display[..display.len() - 1]
                .chars()
                .next_back()
                .is_some_and(crate::linguistics::kana)
        {
            display.pop();
        }
        let units = if let Some(units) = self.edited_units.as_ref().filter(|u| u.raw() == self.raw)
        {
            units.clone()
        } else if self.uses_kana() {
            let mut tokens: Vec<Token> = vec![];
            for (vk, ch, shift) in &self.typed_keys {
                let kana = crate::kana::key(*vk, *shift).unwrap_or(*ch);
                if let Some(last) = tokens.last_mut().filter(|_| "゛゜".contains(kana)) {
                    if let Some(combined) = last
                        .kana
                        .chars()
                        .next_back()
                        .and_then(|c| crate::kana::combine(c, kana))
                    {
                        last.kana.pop();
                        last.kana.push(combined);
                        last.spelling.push(*ch);
                        continue;
                    }
                }
                tokens.push(Token {
                    spelling: ch.to_string(),
                    kana: kana.to_string(),
                });
            }
            Units::from_tokens(tokens)
        } else {
            Units::capture(&self.raw, &display)
        };
        let (tokens, pending) = units.parts();
        let mut english = vec![];
        let mut cursor = 0;
        for word in display
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|s| !s.is_empty())
        {
            if let Some(offset) = self.raw[cursor..].find(word) {
                let start = cursor + offset;
                let end = start + word.len();
                english.push((start, end));
                cursor = end;
            }
        }
        let spans = crate::protection::scan(&self.raw);
        let mut at = 0;
        let blocked: Vec<_> = tokens
            .iter()
            .map(|u| {
                let end = at + u.spelling.len();
                let blocked = english.iter().any(|(a, b)| at < *b && end > *a)
                    || spans.iter().any(|s| at < s.end && end > s.start);
                at = end;
                blocked
            })
            .collect();
        let mut start = 0;
        while start < tokens.len() {
            if blocked[start] {
                start += 1;
                continue;
            }
            let mut end = start;
            let mut reading = String::new();
            let mut offsets = vec![0];
            while end < tokens.len() && !blocked[end] {
                reading.push_str(&tokens[end].kana);
                offsets.push(reading.len());
                end += 1;
            }
            if end == tokens.len() && matches!(pending.to_ascii_lowercase().as_str(), "n" | "nn") {
                reading.push('ん');
                offsets.push(reading.len());
            }
            if let Some(found) = Linguistics::bundled().find_misspelling(&reading) {
                if let (Some(first), Some(last)) = (
                    offsets.iter().position(|p| *p == found.start),
                    offsets.iter().position(|p| *p == found.end),
                ) {
                    if first < last {
                        return Some(Edit {
                            units,
                            start: start + first,
                            end: start + last,
                            right: found.right,
                        });
                    }
                }
            }
            start = end;
        }
        None
    }
    pub(super) fn misspelling_suggestion(&self) -> Option<String> {
        self.composition_misspelling()
            .map(|e| format!("もしかして: {}　<Tab>で修正", e.right))
    }
    pub(super) fn repair_misspelling(&mut self, mut edit: Edit) {
        let (tokens, _) = edit.units.parts();
        let start_key: usize = tokens[..edit.start]
            .iter()
            .map(|u| u.spelling.chars().count())
            .sum();
        let end_key: usize = if edit.end > tokens.len() {
            self.raw.chars().count()
        } else {
            tokens[..edit.end]
                .iter()
                .map(|u| u.spelling.chars().count())
                .sum()
        };
        let right = crate::user_dictionary::hiragana(&edit.right);
        let mut keys = self.typed_keys[..start_key.min(self.typed_keys.len())].to_vec();
        keys.extend(right.chars().map(|c| (0, c, false)));
        keys.extend_from_slice(&self.typed_keys[end_key.min(self.typed_keys.len())..]);
        edit.units.replace_reading(edit.start, edit.end, &right);
        self.raw = edit.units.raw().to_owned();
        if self.uses_kana() {
            self.kana_text = edit.units.kana(false);
        }
        self.typed_keys = keys;
        self.edited_units = Some(edit.units);
        self.explicit = None;
        self.candidates.clear();
        self.clauses.clear();
        self.live_clauses.clear();
        self.selected = 0;
        self.selected_clause = 0;
        self.selected_prediction = -1;
        self.tab_started_conversion = false;
        self.chosen_conversion = false;
    }
}
