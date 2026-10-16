// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use crate::{
    dictionary::Dictionary,
    linguistics::Linguistics,
    romaji,
    settings::Settings,
    storage::{Change, State, Store},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    pub delete_before: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub text: String,
    pub converting: bool,
    pub selected_index: usize,
    pub selected_clause: usize,
    pub hint: String,
    pub candidates: Vec<String>,
    pub clauses: Vec<String>,
    pub meaning: Option<String>,
    pub suggestion: Option<String>,
    pub predictions: Vec<String>,
    pub selected_prediction: i32,
    pub typed: Option<String>,
    pub notes: Vec<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResult {
    pub consumed: bool,
    pub commits: Vec<TextEdit>,
    pub view: Option<View>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub english: BTreeMap<String, bool>,
    pub selected: BTreeMap<String, String>,
    pub user_dictionary: BTreeMap<String, Vec<String>>,
}

pub struct Session {
    raw: String,
    candidates: Vec<String>,
    selected: usize,
    explicit: Option<String>,
    pub direct: bool,
    pub code_input: bool,
    pub private: bool,
    pub preferences: Preferences,
    pub settings: Settings,
    pub state: State,
    store: Option<Store>,
    pub storage_error: Option<String>,
    before: String,
    after: String,
    selected_prediction: i32,
    translation_candidates: Vec<String>,
    function_key: i32,
    case_cycle: u8,
    kana_text: String,
    process: String,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            raw: String::new(),
            candidates: vec![],
            selected: 0,
            explicit: None,
            direct: false,
            code_input: false,
            private: false,
            preferences: Preferences::default(),
            settings: Settings::default(),
            state: State::default(),
            store: None,
            storage_error: None,
            before: String::new(),
            after: String::new(),
            selected_prediction: -1,
            translation_candidates: vec![],
            function_key: 0,
            case_cycle: 0,
            kana_text: String::new(),
            process: String::new(),
        }
    }
}

impl Session {
    pub fn open(store: Store) -> std::io::Result<Self> {
        let settings = store.settings()?;
        let state = store.load()?;
        Ok(Self {
            preferences: state.preferences.clone(),
            state,
            settings,
            store: Some(store),
            selected_prediction: -1,
            ..Default::default()
        })
    }
    pub fn set_context(&mut self, before: Option<&str>, after: Option<&str>) {
        if let Some(before) = before {
            self.before = before
                .chars()
                .rev()
                .take(4000)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
        }
        if let Some(after) = after {
            self.after = after.chars().take(4000).collect();
        }
    }
    pub fn reload(&mut self) -> std::io::Result<()> {
        if let Some(store) = &self.store {
            self.settings = store.settings()?.for_app(&self.process);
            self.state = store.load()?;
            self.preferences = self.state.preferences.clone();
        }
        Ok(())
    }
    pub fn control(&mut self, request: &serde_json::Value) -> std::io::Result<serde_json::Value> {
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| std::io::Error::other("session has no persistent store"))?;
        let result = crate::control::execute(store, request)?;
        self.reload()?;
        Ok(result)
    }
    pub fn set_process(&mut self, process: &str) {
        self.process = process.into();
        let settings = self
            .store
            .as_ref()
            .and_then(|s| s.settings().ok())
            .unwrap_or_else(|| self.settings.clone());
        self.settings = settings.for_app(process);
        self.code_input = self.settings.text("AppProfile") == "Code";
    }
    fn learn(&mut self, changes: Vec<Change>) {
        if self.private || !self.settings.boolean("LearningEnabled") {
            return;
        }
        self.state.preferences = self.preferences.clone();
        if let Some(store) = &self.store {
            match store.record(changes) {
                Ok(state) => {
                    self.state = state;
                    self.preferences = self.state.preferences.clone();
                }
                Err(e) => self.storage_error = Some(e.to_string()),
            }
        } else {
            for change in changes {
                self.state.apply(change);
            }
            self.preferences = self.state.preferences.clone();
        }
    }
    pub fn predictions(&self) -> Vec<String> {
        if self.private
            || !self.settings.boolean("PredictiveCandidates")
            || self.is_converting()
            || self.raw.is_empty()
        {
            return vec![];
        }
        let mut state = self.state.clone();
        state.preferences = self.preferences.clone();
        let mut predictions = Linguistics::bundled().predict(&self.reading(), &state);
        for word in Dictionary::bundled().predict_english(&self.raw) {
            if predictions.len() < 5 && !predictions.contains(&word) {
                predictions.push(word);
            }
        }
        predictions
    }
    pub fn select_prediction(&mut self, index: usize) -> SessionResult {
        let predictions = self.predictions();
        if let Some(text) = predictions.get(index) {
            self.explicit = Some(text.clone());
            self.selected_prediction = -1;
            self.commit()
        } else {
            self.result(false)
        }
    }
    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn reading(&self) -> String {
        if self.settings.text("InputStyle") == "Kana" {
            self.kana_text.clone()
        } else {
            romaji::hiragana(&self.raw, true)
        }
    }
    pub fn is_converting(&self) -> bool {
        !self.candidates.is_empty()
    }
    pub fn display(&self, final_input: bool) -> String {
        if let Some(text) = &self.explicit {
            return text.clone();
        }
        if let Some(text) = self.candidates.get(self.selected) {
            return text.clone();
        }
        if self.settings.text("InputStyle") == "Kana" {
            return self.punctuate(self.kana_text.clone());
        }
        let text = match self.preferences.english.get(&self.raw.to_ascii_lowercase()) {
            Some(true) => self.raw.clone(),
            Some(false) => romaji::hiragana(&self.raw, final_input),
            None => {
                let english_context = self.before.split_whitespace().next_back().is_some_and(|s| {
                    s.len() >= 2
                        && s.chars()
                            .all(|c| c.is_ascii_alphabetic() || ".,!?".contains(c))
                }) && !self.before.chars().any(crate::linguistics::kanji);
                if self.settings.text("DetectionLevel") == "Manual"
                    || self.settings.text("DetectionLevel") == "Conservative"
                {
                    romaji::hiragana(&self.raw, final_input)
                } else {
                    Dictionary::bundled().mixed_context(&self.raw, final_input, english_context)
                }
            }
        };
        let text = crate::protection::transform(&self.raw, |part| {
            if part == self.raw {
                text.clone()
            } else {
                Dictionary::bundled().mixed(part, final_input)
            }
        });
        self.punctuate(text)
    }
    fn punctuate(&self, text: String) -> String {
        let (comma, period) = match self.settings.text("Punctuation") {
            "FullWidthCommaPeriod" => ('，', '．'),
            "FullWidthCommaKuten" => ('，', '。'),
            "ToutenFullWidthPeriod" => ('、', '．'),
            _ => ('、', '。'),
        };
        text.chars()
            .map(|c| match c {
                '、' => comma,
                '。' => period,
                '/' if self.settings.boolean("SlashAsMiddleDot") => '・',
                _ => c,
            })
            .collect()
    }
    pub fn result(&self, consumed: bool) -> SessionResult {
        SessionResult {
            consumed,
            commits: vec![],
            view: if self.raw.is_empty() {
                None
            } else {
                Some(View {
                    text: self.display(false),
                    converting: self.is_converting(),
                    selected_index: self.selected,
                    selected_clause: 0,
                    hint: String::new(),
                    clauses: vec![],
                    candidates: self.candidates.clone(),
                    meaning: if self.settings.boolean("ShowCandidateMeanings")
                        && self.is_converting()
                    {
                        Linguistics::bundled().meaning(&self.display(false), &self.reading())
                    } else {
                        None
                    },
                    suggestion: if self.settings.boolean("CorrectTypos") && !self.is_converting() {
                        Linguistics::bundled().misspelling(&self.reading())
                    } else {
                        None
                    },
                    predictions: self.predictions(),
                    selected_prediction: if self.selected_prediction >= 0 {
                        self.selected_prediction
                    } else {
                        -1
                    },
                    typed: if self.settings.boolean("ShowTypedKeys") {
                        Some(self.raw.clone())
                    } else {
                        None
                    },
                    notes: self
                        .candidates
                        .iter()
                        .map(|c| {
                            if self.translation_candidates.contains(c) {
                                Some("英訳".into())
                            } else {
                                None
                            }
                        })
                        .collect(),
                })
            },
        }
    }
    pub fn reset(&mut self) {
        self.raw.clear();
        self.candidates.clear();
        self.explicit = None;
        self.selected = 0;
        self.selected_prediction = -1;
        self.translation_candidates.clear();
        self.function_key = 0;
        self.case_cycle = 0;
        self.kana_text.clear();
    }
    pub fn commit(&mut self) -> SessionResult {
        let mut text = self.display(true);
        if self.selected_prediction >= 0 {
            if let Some(prediction) = self.predictions().get(self.selected_prediction as usize) {
                text = prediction.clone();
            }
        }
        let reading = self.reading();
        if !self.private && !self.raw.is_empty() {
            let mut changes = vec![Change::Phrase {
                reading: reading.clone(),
                text: text.clone(),
            }];
            if self.is_converting() {
                changes.push(Change::Conversion {
                    reading: reading.clone(),
                    text: text.clone(),
                    translation: self.translation_candidates.contains(&text),
                });
            }
            if self.function_key != 0 {
                changes.push(Change::Language {
                    raw: self.raw.clone(),
                    english: self.function_key >= 0x78,
                    explicit: true,
                });
            }
            self.learn(changes);
        }
        if self.settings.boolean("SpaceAroundEnglish") {
            text = space_english(&text);
        }
        self.before.push_str(&text);
        if self.before.chars().count() > 4000 {
            self.before = self
                .before
                .chars()
                .rev()
                .take(4000)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
        }
        self.reset();
        SessionResult {
            consumed: !text.is_empty(),
            commits: if text.is_empty() {
                vec![]
            } else {
                vec![TextEdit {
                    delete_before: 0,
                    text,
                }]
            },
            view: None,
        }
    }
    pub fn select(&mut self, index: usize) -> SessionResult {
        if index >= self.candidates.len() {
            return self.result(false);
        }
        self.selected = index;
        self.commit()
    }
    pub fn begin_conversion(&mut self, external: Vec<String>) {
        if self.raw.is_empty() || self.is_converting() {
            return;
        }
        let reading = self.reading();
        let mut candidates = self
            .preferences
            .user_dictionary
            .get(&reading)
            .cloned()
            .unwrap_or_default();
        candidates.extend(external);
        candidates.extend(Dictionary::bundled().candidates(&self.display(true)));
        candidates.extend(Dictionary::bundled().candidates(&reading));
        candidates.extend(Linguistics::bundled().words(&reading));
        if let Some(context) =
            Linguistics::bundled().context(&reading, &format!("{}{}", self.before, self.after))
        {
            candidates.insert(0, context);
        }
        if self.settings.boolean("TranslationCandidates") {
            self.translation_candidates = Linguistics::bundled().translations(
                candidates.first().map(String::as_str).unwrap_or(&reading),
                &reading,
            );
            candidates.extend(self.translation_candidates.clone());
        }
        candidates.push(self.raw.clone());
        let mut unique = Vec::new();
        for candidate in candidates {
            if !candidate.is_empty() && !unique.contains(&candidate) {
                unique.push(candidate);
            }
        }
        if let Some(learned) = self.preferences.selected.get(&reading) {
            if let Some(i) = unique.iter().position(|s| s == learned) {
                let value = unique.remove(i);
                unique.insert(0, value);
            }
        }
        self.candidates = unique;
        self.selected = 0;
        self.explicit = None;
        self.selected_prediction = -1;
    }
    /// Virtual keys follow the existing native ABI; modifiers: shift=1, ctrl=2,
    /// alt=4, command=8. Platform shortcuts commit first and pass through.
    pub fn key(&mut self, vk: i32, ch: i32, modifiers: i32) -> SessionResult {
        if ch < 0 || char::from_u32(ch as u32).is_none() {
            return self.result(false);
        }
        let code_direct = self.code_input
            && crate::context::classify(&self.before) == crate::context::LineKind::Code;
        if modifiers & 14 != 0
            || self.direct
            || self.private
            || code_direct
            || !self.settings.boolean("Enabled")
        {
            let mut result = self.commit();
            result.consumed = false;
            return result;
        }
        match vk {
            0x10..=0x12 | 0xa0..=0xa5 | 0x5b..=0x5c => self.result(false),
            0x1b => {
                if self.raw.is_empty() {
                    return self.result(false);
                }
                if self.is_converting() {
                    self.candidates.clear();
                    self.explicit = None;
                } else {
                    self.reset();
                }
                self.result(true)
            }
            0x08 => {
                if self.raw.is_empty() {
                    return self.result(false);
                }
                self.candidates.clear();
                self.explicit = None;
                self.raw.pop();
                self.kana_text.pop();
                self.result(true)
            }
            0x0d => self.commit(),
            0x20 | 0x09
                if self.raw.starts_with(self.settings.text("SnippetMark"))
                    && !self.settings.text("SnippetMark").is_empty()
                    && self
                        .state
                        .snippets
                        .contains_key(&self.raw[self.settings.text("SnippetMark").len()..]) =>
            {
                self.explicit = self
                    .state
                    .snippets
                    .get(&self.raw[self.settings.text("SnippetMark").len()..])
                    .cloned();
                self.commit()
            }
            0x09 if !self.raw.is_empty() && !self.is_converting() => {
                if let Some(suggestion) = self.result(false).view.and_then(|v| v.suggestion) {
                    self.explicit = Some(crate::user_dictionary::hiragana(&suggestion));
                    return self.result(true);
                }
                let predictions = self.predictions();
                if !predictions.is_empty() {
                    let direction = if modifiers & 1 != 0 { -1 } else { 1 };
                    self.selected_prediction =
                        (self.selected_prediction + direction).rem_euclid(predictions.len() as i32);
                    return self.result(true);
                }
                if self.settings.boolean("TabConversion") {
                    self.begin_conversion(vec![]);
                    return self.result(true);
                }
                let mut result = self.commit();
                result.consumed = false;
                result
            }
            0x1c if self.settings.boolean("SeparateKeyRoles") && !self.raw.is_empty() => {
                self.begin_conversion(vec![]);
                self.result(true)
            }
            0x1d if self.settings.boolean("SeparateKeyRoles") && !self.raw.is_empty() => {
                self.key(0x79, 0, modifiers)
            }
            0x15 if self.settings.boolean("SeparateKeyRoles") && !self.raw.is_empty() => self.key(
                if self.function_key == 0x75 {
                    0x76
                } else {
                    0x75
                },
                0,
                modifiers,
            ),
            0x20 if !self.raw.is_empty() => {
                if !self.is_converting()
                    && !self.settings.boolean("SeparateKeyRoles")
                    && (self.display(true) == self.raw
                        || self.before.is_empty()
                            && Dictionary::bundled().is_common_english(&self.raw))
                    && Dictionary::bundled().is_english(&self.raw)
                {
                    self.explicit = Some(self.raw.clone() + " ");
                    return self.commit();
                }
                if self.is_converting() {
                    self.cycle(if modifiers & 1 != 0 { -1 } else { 1 });
                } else {
                    self.begin_conversion(vec![]);
                }
                self.result(true)
            }
            0x26 | 0x28 | 0x09 if self.is_converting() => {
                self.cycle(if vk == 0x26 || modifiers & 1 != 0 {
                    -1
                } else {
                    1
                });
                self.result(true)
            }
            0x75..=0x79 if !self.raw.is_empty() => {
                self.candidates.clear();
                self.case_cycle = if self.function_key == vk {
                    (self.case_cycle + 1) % 3
                } else {
                    0
                };
                self.function_key = vk;
                let raw = match self.case_cycle {
                    1 => self.raw.to_ascii_uppercase(),
                    2 => {
                        let mut raw = self.raw.to_ascii_lowercase();
                        if let Some(first) = raw.get_mut(..1) {
                            first.make_ascii_uppercase();
                        }
                        raw
                    }
                    _ => self.raw.clone(),
                };
                let reading = self.reading();
                self.explicit = Some(crate::protection::transform(&raw, |part| match vk {
                    0x75 => {
                        if self.settings.text("InputStyle") == "Kana" {
                            reading.clone()
                        } else {
                            romaji::hiragana(part, true)
                        }
                    }
                    0x76 => romaji::katakana(&romaji::hiragana(part, true)),
                    0x77 => {
                        crate::kana::halfwidth(&romaji::katakana(&romaji::hiragana(part, true)))
                    }
                    0x78 => crate::kana::fullwidth(part),
                    _ => part.into(),
                }));
                self.result(true)
            }
            _ if ch >= 0x20 && ch != 0x7f => {
                // Retain the old commit when a new character starts after conversion.
                let commits =
                    if self.is_converting() && !self.settings.boolean("ContinueAfterConversion") {
                        self.commit().commits
                    } else {
                        self.candidates.clear();
                        vec![]
                    };
                self.explicit = None;
                self.selected_prediction = -1;
                self.function_key = 0;
                if self.raw.chars().count() >= 1024 {
                    let mut result = self.commit();
                    result.consumed = false;
                    return result;
                }
                self.raw.push(char::from_u32(ch as u32).unwrap());
                if self.settings.text("InputStyle") == "Kana" {
                    if let Some(kana) = crate::kana::key(vk, modifiers & 1 != 0) {
                        if "゛゜".contains(kana) {
                            if let Some(last) = self.kana_text.chars().next_back() {
                                if let Some(combined) = crate::kana::combine(last, kana) {
                                    self.kana_text.pop();
                                    self.kana_text.push(combined);
                                } else {
                                    self.kana_text.push(kana);
                                }
                            } else {
                                self.kana_text.push(kana);
                            }
                        } else {
                            self.kana_text.push(kana);
                        }
                    } else {
                        self.kana_text.push(char::from_u32(ch as u32).unwrap());
                    }
                }
                let mut result = self.result(true);
                result.commits = commits;
                result
            }
            _ => {
                let mut result = self.commit();
                result.consumed = false;
                result
            }
        }
    }
    fn cycle(&mut self, direction: i32) {
        self.selected =
            (self.selected as i32 + direction).rem_euclid(self.candidates.len() as i32) as usize;
    }
}

fn space_english(text: &str) -> String {
    let mut out = String::new();
    let chars: Vec<_> = text.chars().collect();
    for (i, c) in chars.iter().copied().enumerate() {
        if i > 0 {
            let previous = chars[i - 1];
            if (previous.is_ascii_alphabetic()
                && (crate::linguistics::kana(c) || crate::linguistics::kanji(c)))
                || ((crate::linguistics::kana(previous) || crate::linguistics::kanji(previous))
                    && c.is_ascii_alphabetic())
            {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out
}
