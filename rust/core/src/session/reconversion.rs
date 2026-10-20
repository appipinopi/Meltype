// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit reconversion of text typed while the IME was off.
use super::{romaji, Clause, Session, SessionResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconversionPart {
    pub text: String,
    pub english: bool,
}
impl Session {
    /// Plan the original ConvertRomajiText operation without calling a host.
    /// Consecutive Japanese words are one converter request; English retains
    /// word boundaries. The native caller resolves requests outside its lock.
    pub fn romaji_reconversion_parts(&self, text: &str) -> Option<Vec<ReconversionPart>> {
        if !self.raw.is_empty()
            || self.private
            || !self.settings.boolean("Enabled")
            || text.chars().count() > 1024
            || !text.bytes().any(|b| b.is_ascii_alphabetic())
            || !text.bytes().all(|b| (b' '..=b'~').contains(&b))
        {
            return None;
        }
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        let dict = self.dictionary();
        let mut parts: Vec<ReconversionPart> = Vec::new();
        let mut previous_english = false;
        for word in text.split_whitespace() {
            let strict = romaji::analyze(&word.to_ascii_lowercase(), false);
            // Two-letter readable words need evidence on both sides. A
            // preceding English word alone must not turn PR no into PR no.
            let context = previous_english
                && !(word.len() <= 2
                    && strict.valid
                    && ["", "n"].contains(&strict.partial.as_str()));
            let mut detected = self
                .punctuate_with_context(dict.mixed_context(word, true, context), previous_english);
            let all_english = detected.is_ascii()
                && detected.bytes().any(|b| b.is_ascii_alphabetic())
                && (dict.rendered_english_tail(&detected, "Balanced")
                    || dict.unknown_english_word(&word.to_ascii_lowercase())
                    || !crate::protection::scan(&detected).is_empty());
            // ConvertRomajiText reads CompositionSegment.Kana directly,
            // unlike normal RenderSegments. Its final Japanese Pending
            // (including n) is not part of the converter request.
            let fragment = romaji::analyze(&word.to_ascii_lowercase(), true);
            if !all_english
                && !dict.rendered_english_tail(&detected, "Balanced")
                && fragment.valid
                && !fragment.partial.is_empty()
            {
                let pending = if fragment.partial == "n" {
                    "ん"
                } else {
                    &fragment.partial
                };
                if detected.ends_with(pending) {
                    detected.truncate(detected.len() - pending.len());
                }
                previous_english = false;
            }
            let mut at = 0;
            while at < detected.len() {
                let ascii = detected[at..].chars().next().unwrap().is_ascii();
                let end = detected[at..]
                    .char_indices()
                    .find(|(_, c)| c.is_ascii() != ascii)
                    .map_or(detected.len(), |(i, _)| at + i);
                let run = &detected[at..end];
                let english = ascii
                    && (all_english
                        || !crate::protection::scan(run).is_empty()
                        || dict.rendered_english_tail(&detected[..end], "Balanced"));
                if english {
                    for word in dict.english_parts(run) {
                        parts.push(ReconversionPart {
                            text: word.into(),
                            english: true,
                        });
                    }
                } else if let Some(last) = parts.last_mut().filter(|p| !p.english) {
                    last.text.push_str(run);
                } else {
                    parts.push(ReconversionPart {
                        text: run.into(),
                        english: false,
                    });
                }
                previous_english = english;
                at = end;
            }
        }
        Some(parts)
    }
    /// Begin only after the host has resolved the conversion plan. The host
    /// must validate selection_expect before substituting a preedit.
    pub fn begin_romaji_reconversion(&mut self, original: &str, converted: &str) -> SessionResult {
        if self.romaji_reconversion_parts(original).is_none()
            || converted.is_empty()
            || converted == original
            || converted.chars().count() > 8192
        {
            return self.result(false);
        }
        self.forget_direct_input();
        self.correctable.clear();
        self.raw = converted.into();
        self.typed_keys = converted.chars().map(|c| (0, c, false)).collect();
        self.kana_text = converted.into();
        self.clauses = vec![Clause::new(
            converted.into(),
            vec![converted.into(), original.into()],
        )];
        self.selected_clause = 0;
        self.selected_prediction = -1;
        self.explicit = None;
        self.function_key = 0;
        self.case_cycle = 0;
        self.reconversion_origin = Some(original.into());
        self.sync_clause();
        let mut result = self.result(true);
        result.selection_expect = Some(original.into());
        result
    }
}
