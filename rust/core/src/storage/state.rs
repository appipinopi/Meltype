// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Learning data shared by filesystem and browser hosts.
use super::now;
use crate::session::Preferences;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub prefix_model: crate::prefix_learning::Model,
    pub preferences: Preferences,
    pub language_counts: BTreeMap<String, LanguageEntry>,
    pub translations: BTreeMap<String, BTreeMap<String, u32>>,
    pub phrases: Vec<Phrase>,
    pub snippets: BTreeMap<String, String>,
    pub used: BTreeMap<String, i64>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LanguageEntry {
    pub english: bool,
    pub count: u32,
    pub explicit: bool,
    pub used: i64,
}
impl LanguageEntry {
    pub fn active(&self, word: &str) -> bool {
        let word = word
            .strip_suffix("@e")
            .or_else(|| word.strip_suffix("@j"))
            .unwrap_or(word);
        if !self.english || self.count.max(1) >= 2 {
            return true;
        }
        let analysis = crate::romaji::analyze(word, true);
        !(crate::typo::Corrector::bundled().is_common_japanese(word)
            || !self.explicit
                && word.len() <= 2
                && analysis.valid
                && (analysis.partial.is_empty() || analysis.partial == "n"))
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Phrase {
    pub reading: String,
    pub text: String,
    pub count: u32,
    pub used: i64,
}

#[derive(Clone)]
pub enum Change {
    Language {
        raw: String,
        english: bool,
        explicit: bool,
    },
    ContextualLanguage {
        raw: String,
        english: bool,
        explicit: bool,
        context: bool,
    },
    Conversion {
        reading: String,
        text: String,
        translation: bool,
    },
    Phrase {
        reading: String,
        text: String,
    },
}
impl State {
    pub(super) fn rebuild_language_preferences(&mut self) {
        for (word, entry) in &mut self.language_counts {
            entry.count = entry.count.max(1);
            if entry.active(
                word.strip_suffix("@e")
                    .or_else(|| word.strip_suffix("@j"))
                    .unwrap_or(word),
            ) {
                self.preferences.english.insert(word.clone(), entry.english);
            } else {
                self.preferences.english.remove(word);
            }
        }
        if self.language_counts.len() > 3000 {
            let mut oldest: Vec<_> = self
                .language_counts
                .iter()
                .map(|(word, entry)| (entry.used, word.clone()))
                .collect();
            oldest.sort();
            for (_, word) in oldest.into_iter().take(self.language_counts.len() - 2700) {
                self.language_counts.remove(&word);
                self.preferences.english.remove(&word);
                self.used.remove(&format!("language:{word}"));
            }
        }
    }
    fn remember_language(
        &mut self,
        raw: String,
        english: bool,
        explicit: bool,
        context: Option<bool>,
    ) {
        let raw = raw.to_ascii_lowercase();
        if raw.len() < 2 || !raw.bytes().all(|c| c.is_ascii_lowercase()) {
            return;
        }
        let spelling = raw.clone();
        let raw = match context {
            Some(true) => raw + "@e",
            Some(false) => raw + "@j",
            None => raw,
        };
        let entry = self.language_counts.entry(raw.clone()).or_default();
        if entry.english != english {
            *entry = LanguageEntry::default();
        }
        entry.english = english;
        entry.count = entry.count.saturating_add(1);
        entry.explicit |= explicit;
        entry.used = now();
        if entry.active(&spelling) {
            self.preferences.english.insert(raw.clone(), english);
        } else {
            self.preferences.english.remove(&raw);
        }
        self.used.insert(format!("language:{raw}"), now());
        if self.language_counts.len() > 3000 {
            self.rebuild_language_preferences();
        }
    }
    pub fn apply(&mut self, change: Change) {
        match change {
            Change::Language {
                raw,
                english,
                explicit,
            } => {
                self.remember_language(raw, english, explicit, None);
            }
            Change::ContextualLanguage {
                raw,
                english,
                explicit,
                context,
            } => {
                self.remember_language(raw, english, explicit, Some(context));
            }
            Change::Conversion {
                reading,
                text,
                translation,
            } => {
                if !translation
                    && (crate::time_candidates::is_dynamic_choice(&reading, &text)
                        || reading.encode_utf16().count() < 2
                        || text.is_empty())
                {
                    return;
                }
                if translation {
                    let count = self
                        .translations
                        .entry(reading.clone())
                        .or_default()
                        .entry(text)
                        .or_default();
                    *count = count.saturating_add(1);
                } else {
                    self.preferences.selected.insert(reading.clone(), text);
                }
                self.used.insert(format!("conversion:{reading}"), now());
            }
            Change::Phrase { reading, text } => {
                if crate::time_candidates::is_dynamic_choice(&reading, &text) {
                    return;
                }
                if reading.chars().count() < 3
                    || reading == text
                    || text.is_empty()
                    || text.contains(['\t', '\n', '\r'])
                {
                    return;
                }
                if let Some(old) = self
                    .phrases
                    .iter_mut()
                    .find(|p| p.reading == reading && p.text == text)
                {
                    old.count = old.count.saturating_add(1);
                    old.used = now();
                } else {
                    self.phrases.push(Phrase {
                        reading,
                        text,
                        count: 1,
                        used: now(),
                    });
                }
                if self.phrases.len() > 3000 {
                    self.phrases.sort_by_key(|p| std::cmp::Reverse(p.used));
                    self.phrases.truncate(2700);
                }
            }
        }
        if self.used.len() > 10000 {
            let mut oldest: Vec<_> = self.used.iter().map(|(k, t)| (k.clone(), *t)).collect();
            oldest.sort_by_key(|(_, t)| *t);
            for (key, _) in oldest.into_iter().take(1000) {
                self.remove_learned(&key);
            }
        }
    }
    pub fn remove_learned(&mut self, key: &str) {
        if let Some(prefix) = key.strip_prefix("prefix:") {
            self.prefix_model.prefixes.remove(prefix);
        }
        if let Some(raw) = key.strip_prefix("language:") {
            self.preferences.english.remove(raw);
            self.language_counts.remove(raw);
        }
        if let Some(reading) = key.strip_prefix("conversion:") {
            self.preferences.selected.remove(reading);
            self.translations.remove(reading);
        }
        if let Some(reading) = key.strip_prefix("phrase:") {
            self.phrases.retain(|phrase| phrase.reading != reading);
        }
        self.used.remove(key);
    }
    pub fn forget_since(&mut self, since: i64) -> usize {
        let keys: Vec<_> = self
            .used
            .iter()
            .filter(|(_, time)| **time >= since)
            .map(|(key, _)| key.clone())
            .collect();
        let count = keys.len();
        for key in keys {
            self.remove_learned(&key);
        }
        let old = self.phrases.len();
        self.phrases.retain(|p| p.used < since);
        count + old - self.phrases.len()
    }
    pub fn clear_learning(&mut self) {
        self.prefix_model.prefixes.clear();
        self.preferences.english.clear();
        self.preferences.selected.clear();
        self.language_counts.clear();
        self.translations.clear();
        self.phrases.clear();
        self.used.clear();
    }
}
