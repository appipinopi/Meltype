// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
mod clause_metadata;
mod direct;
mod engine_learning;
mod reconversion;
use crate::{
    dictionary::Dictionary,
    linguistics::Linguistics,
    romaji,
    settings::Settings,
    storage::{Change, State, Store},
};
pub use engine_learning::Batch as EngineLearning;
pub use reconversion::ReconversionPart;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    pub delete_before: usize,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expect: Option<String>,
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
    #[serde(rename = "selectionExpect", skip_serializing_if = "Option::is_none")]
    pub selection_expect: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub english: BTreeMap<String, bool>,
    pub selected: BTreeMap<String, String>,
    pub user_dictionary: BTreeMap<String, Vec<String>>,
}

/// One converter segment. Reading boundaries are Unicode scalar boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clause {
    pub reading: String,
    pub candidates: Vec<String>,
    pub selected: usize,
    /// English is preserved until the user explicitly moves its boundary.
    pub english: bool,
    /// None for protected text, Some for English sound-unit readings.
    pub kana: Option<String>,
    /// Original input for this exact reading range; cleared by boundary edits.
    pub raw: Option<String>,
    /// True only after an explicit candidate selection; reset by reconversion.
    pub changed: bool,
    /// Translation membership belongs to this clause, not the whole sentence.
    pub translations: Vec<String>,
}
impl Clause {
    pub fn new(reading: String, mut candidates: Vec<String>) -> Self {
        candidates.retain(|c| !c.is_empty());
        let mut unique = Vec::new();
        for candidate in candidates {
            if !unique.contains(&candidate) {
                unique.push(candidate);
            }
        }
        // Converter replies retain their order. The session adds kana/raw
        // alternatives when building the final list; injecting a reading here
        // would place it ahead of dictionary words and shift numbered choices.
        if unique.is_empty() {
            unique.push(reading.clone());
        }
        Self {
            reading,
            candidates: unique,
            selected: 0,
            english: false,
            kana: None,
            raw: None,
            changed: false,
            translations: Vec::new(),
        }
    }
    pub fn literal(reading: String, ordinary_english: bool) -> Self {
        let mut clause = Self::new(reading.clone(), vec![reading]);
        clause.english = true;
        if ordinary_english {
            clause.kana = Some(String::new());
        }
        clause
    }
    pub fn text(&self) -> &str {
        &self.candidates[self.selected]
    }
}

struct CommittedInput {
    raw: String,
    text: String,
    keys: Vec<(i32, char, bool)>,
    units: Option<crate::composition_units::Units>,
    romaji: Option<std::sync::Arc<romaji::CustomTable>>,
}

pub struct Session {
    raw: String,
    candidates: Vec<String>,
    selected: usize,
    clauses: Vec<Clause>,
    live_clauses: Vec<Clause>,
    selected_clause: usize,
    explicit: Option<String>,
    pub direct: bool,
    pub external_converter_only: bool,
    pub automatic_english_spacing: bool,
    pub direct_detection: bool,
    pub direct_english_word: bool,
    pub code_input: bool,
    code_japanese: bool,
    pub private: bool,
    /// InputScope.PRIVATE permits conversion while suppressing learning.
    pub no_learning: bool,
    engine_learning: Option<Vec<engine_learning::Batch>>,
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
    pub can_delete: bool,
    last_commit: Option<CommittedInput>,
    typed_keys: Vec<(i32, char, bool)>,
    reconversion_origin: Option<String>,
    context_is_fresh: bool,
    composition_kana: bool,
    correctable: Vec<crate::commit_correction::Record>,
    space_started_conversion: bool,
    chosen_conversion: bool,
    sigil: crate::sigil::Sigil,
    edited_units: Option<crate::composition_units::Units>,
    custom_romaji: Option<std::sync::Arc<romaji::CustomTable>>,
    clock: std::time::Instant,
    key_time_ms: u64,
    direct_pending: String,
    direct_last_key_ms: Option<u64>,
    hook_session: crate::input_session::InputSession,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            raw: String::new(),
            candidates: vec![],
            selected: 0,
            clauses: vec![],
            live_clauses: vec![],
            selected_clause: 0,
            explicit: None,
            direct: false,
            external_converter_only: false,
            automatic_english_spacing: false,
            direct_detection: false,
            direct_english_word: false,
            code_input: false,
            code_japanese: false,
            private: false,
            no_learning: false,
            engine_learning: None,
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
            can_delete: false,
            last_commit: None,
            typed_keys: vec![],
            reconversion_origin: None,
            context_is_fresh: false,
            composition_kana: false,
            correctable: vec![],
            space_started_conversion: false,
            chosen_conversion: false,
            sigil: Default::default(),
            edited_units: None,
            custom_romaji: None,
            clock: std::time::Instant::now(),
            key_time_ms: 0,
            direct_pending: String::new(),
            direct_last_key_ms: None,
            hook_session: Default::default(),
        }
    }
}

impl Session {
    pub fn open(store: Store) -> std::io::Result<Self> {
        let settings = store.settings()?;
        let state = store.load()?;
        let (custom_romaji, storage_error) =
            match romaji::CustomTable::load(&store.directory.join("dictionaries")) {
                Ok(table) => (table, None),
                Err(error) => (None, Some(format!("romaji.txt: {error}"))),
            };
        let mut hook_session = crate::input_session::InputSession::default();
        hook_session.set_model(state.prefix_model.clone());
        Ok(Self {
            hook_session,
            preferences: state.preferences.clone(),
            state,
            settings,
            store: Some(store),
            selected_prediction: -1,
            custom_romaji,
            storage_error,
            ..Default::default()
        })
    }
    pub fn set_context(&mut self, before: Option<&str>, after: Option<&str>) {
        if self.private {
            self.before.clear();
            self.after.clear();
            self.context_is_fresh = false;
            return;
        }
        self.context_is_fresh = before.is_some();
        if let Some(before) = before {
            self.sigil.context(before);
            let expected: String = self.correctable.iter().map(|r| r.text.as_str()).collect();
            if !before.ends_with(&expected) {
                self.correctable.clear();
            }
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
            self.hook_session.set_model(self.state.prefix_model.clone());
            self.preferences = self.state.preferences.clone();
            if self.raw.is_empty() {
                match romaji::CustomTable::load(&store.directory.join("dictionaries")) {
                    Ok(table) => self.custom_romaji = table,
                    Err(error) => self.storage_error = Some(format!("romaji.txt: {error}")),
                }
            }
        }
        Ok(())
    }
    pub fn control(&mut self, request: &serde_json::Value) -> std::io::Result<serde_json::Value> {
        if request["op"] == "prefix_learn" && (self.private || self.no_learning) {
            return Ok(serde_json::json!({"saved":false}));
        }
        if request["op"] == "hook_session" {
            let command = serde_json::from_value(request["command"].clone())?;
            let _romaji = romaji::Scope::new(self.custom_romaji.clone());
            return Ok(serde_json::to_value(
                self.hook_session.execute(command, &self.settings),
            )?);
        }
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| std::io::Error::other("session has no persistent store"))?;
        let result = crate::control::execute(store, request)?;
        self.reload()?;
        Ok(result)
    }
    pub fn set_process(&mut self, process: &str) {
        if self.process != process {
            self.forget_direct_input();
            self.sigil = Default::default();
            self.correctable.clear();
            self.code_japanese = false;
            self.last_commit = None;
        }
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
        if self.private || self.no_learning || !self.settings.boolean("LearningEnabled") {
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
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
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
    pub fn code_english(&self) -> bool {
        self.code_input
            && !self.code_japanese
            && !self.direct
            && crate::context::classify(&self.before) == crate::context::LineKind::Code
    }
    pub fn set_code_line(&mut self, japanese: bool) -> bool {
        if !self.code_input || self.direct || !self.raw.is_empty() {
            return false;
        }
        if japanese {
            if self.code_japanese
                || crate::context::classify(&self.before) != crate::context::LineKind::Code
            {
                return false;
            }
            self.code_japanese = true;
        } else {
            if !self.code_japanese {
                return false;
            }
            self.code_japanese = false;
        }
        true
    }
    pub fn caret_moved(&mut self, vk: i32, control: bool) {
        self.sigil.lose();
        self.correctable.clear();
        self.last_commit = None;
        self.forget_direct_input();
        if control || matches!(vk, 0 | 9 | 0x21..=0x28 | 0x2e) {
            self.code_japanese = false;
        }
    }
    pub fn spelling_probe(
        &self,
        vk: i32,
        ch: i32,
        modifiers: i32,
        before: Option<&str>,
        after: Option<&str>,
    ) -> Option<crate::spelling::Probe> {
        if self.private
            || self.direct
            || self.code_english()
            || self.uses_kana()
            || !self.settings.boolean("Enabled")
            || self.settings.text("DetectionLevel") == "Manual"
        {
            return None;
        }
        let mut words = vec![self.raw.clone()];
        if vk == -1 {
            words.extend(self.last_commit.iter().map(|commit| commit.raw.clone()));
        }
        if modifiers & 14 == 0 {
            if vk == 8 {
                let mut raw = self.raw.clone();
                raw.pop();
                words.push(raw);
            } else if let Some(c) = u32::try_from(ch)
                .ok()
                .and_then(char::from_u32)
                .filter(|c| *c >= ' ' && *c != '\u{7f}')
            {
                words.push(format!("{}{c}", self.raw));
            }
        }
        Some(crate::spelling::Probe {
            romaji: self.custom_romaji.clone(),
            words,
            before: before
                .unwrap_or(&self.before)
                .chars()
                .rev()
                .take(4000)
                .collect::<String>()
                .chars()
                .rev()
                .collect(),
            after: after.unwrap_or(&self.after).chars().take(4000).collect(),
            level: self.settings.text("DetectionLevel").to_owned(),
            scored: Dictionary::uses_scored(self.settings.boolean("ScoredSegmentation")),
        })
    }
    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn reading(&self) -> String {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        if self.reconversion_origin.is_some() {
            self.raw.clone()
        } else if self.uses_kana() {
            self.kana_text.clone()
        } else if let Some(units) = self.edited_units.as_ref().filter(|u| u.raw() == self.raw) {
            if self.external_converter_only {
                if Dictionary::uses_scored(self.settings.boolean("ScoredSegmentation")) {
                    self.mixed_reading(true)
                } else {
                    units.render(&self.mixed_reading(true), true)
                }
            } else {
                units.kana(true)
            }
        } else if let Some(fix) = self.romaji_fix() {
            fix.kana
        } else if self.external_converter_only
            && self.function_key == 0
            && self.settings.text("DetectionLevel") != "Manual"
            && self.preferences.english.get(&self.raw.to_ascii_lowercase()) != Some(&false)
        {
            // Classify only unprotected gaps; a mention must not make the
            // surrounding Japanese sentence look like one protected token.
            self.mixed_reading(true)
        } else {
            crate::protection::transform(&self.raw, |part| romaji::hiragana(part, true))
        }
    }
    /// Conversion engines receive Japanese readings and literal English spans.
    /// This is separate from the reading used by edit and repair operations.
    pub fn conversion_reading(&self) -> String {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        if self.reconversion_origin.is_some() || self.uses_kana() {
            self.reading()
        } else {
            self.mixed_reading(true)
        }
    }
    fn mixed_reading(&self, final_input: bool) -> String {
        crate::protection::transform(&self.raw, |part| self.detect_composition(part, final_input))
    }
    fn detect_composition(&self, raw: &str, final_input: bool) -> String {
        let dictionary = Dictionary::bundled();
        if Dictionary::uses_scored(self.settings.boolean("ScoredSegmentation")) {
            dictionary.composition_scored_units(
                raw,
                final_input,
                &self.before,
                &self.after,
                self.settings.text("DetectionLevel"),
                self.edited_units.as_ref().filter(|u| u.raw() == raw),
            )
        } else {
            dictionary.composition(
                raw,
                final_input,
                &self.before,
                &self.after,
                self.settings.text("DetectionLevel"),
            )
        }
    }
    fn refresh_kana(&mut self) {
        if !["Kana", "Both"].contains(&self.settings.text("InputStyle")) {
            return;
        }
        self.kana_text.clear();
        for (vk, ch, shift) in &self.typed_keys {
            let kana = crate::kana::key(*vk, *shift).unwrap_or(*ch);
            if "゛゜".contains(kana) {
                if let Some(combined) = self
                    .kana_text
                    .chars()
                    .next_back()
                    .and_then(|previous| crate::kana::combine(previous, kana))
                {
                    self.kana_text.pop();
                    self.kana_text.push(combined);
                    continue;
                }
            }
            self.kana_text.push(kana);
        }
        if self.settings.text("InputStyle") == "Both" && self.raw.is_ascii() {
            let keys: Vec<_> = self.typed_keys.iter().map(|(vk, _, _)| *vk).collect();
            let score = crate::detection::evaluate(
                &self.raw.to_ascii_lowercase(),
                &keys,
                false,
                &self.settings,
            );
            let kana_score: i32 = score
                .contributions
                .iter()
                .filter(|c| c.source == "Kana")
                .map(|c| c.japanese)
                .sum();
            let romaji_score: i32 = score
                .contributions
                .iter()
                .filter(|c| c.source != "Kana")
                .map(|c| c.japanese)
                .sum();
            self.composition_kana = kana_score >= 2 && kana_score > romaji_score;
        }
    }
    fn uses_kana(&self) -> bool {
        self.settings.text("InputStyle") == "Kana"
            || self.settings.text("InputStyle") == "Both" && self.composition_kana
    }
    fn romaji_fix(&self) -> Option<crate::typo::Fix> {
        if !self.settings.boolean("CorrectTypos")
            || self.preferences.english.get(&self.raw.to_ascii_lowercase()) == Some(&true)
            || !self.raw.bytes().all(|c| c.is_ascii_lowercase())
        {
            return None;
        }
        let corrector = crate::typo::Corrector::bundled();
        let fix = corrector.fix(&self.raw, true)?;
        // English word + Japanese particle/suffix is intentional mixed input.
        // Do not reinterpret that word while repairing a following syllable.
        let dictionary = Dictionary::bundled();
        let mixed = dictionary.mixed(&self.raw, true);
        if mixed == self.raw
            || mixed.split(|c: char| !c.is_ascii_alphabetic()).any(|word| {
                word.len() >= 4 || word.len() >= 3 && dictionary.is_common_english(word)
            })
        {
            return None;
        }
        for end in 2..self.raw.len() {
            let (word, rest) = self.raw.split_at(end);
            if dictionary.is_english(word)
                && (dictionary.is_japanese(rest)
                    || [
                        "ha", "ga", "wo", "ni", "de", "to", "no", "mo", "ya", "wa", "he",
                    ]
                    .contains(&rest)
                    || corrector.is_common_japanese(rest))
            {
                return None;
            }
        }
        Some(fix)
    }
    /// Explicit Latin display commits on Space unless separate key roles request conversion.
    pub fn space_commits_alphanumeric(&self) -> bool {
        !self.is_converting()
            && !self.settings.boolean("SeparateKeyRoles")
            && matches!(self.function_key, 0x78 | 0x79)
    }
    pub fn is_converting(&self) -> bool {
        !self.candidates.is_empty()
    }
    pub fn display(&self, final_input: bool) -> String {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        if let Some(text) = &self.explicit {
            return text.clone();
        }
        if !self.clauses.is_empty() {
            return self.clauses.iter().map(Clause::text).collect();
        }
        if let Some(text) = self.candidates.get(self.selected) {
            return text.clone();
        }
        if !self.live_clauses.is_empty() {
            return self.live_clauses.iter().map(Clause::text).collect();
        }
        if self.uses_kana() {
            return self.punctuate(self.kana_text.clone());
        }
        if final_input {
            if let Some(fix) = self.romaji_fix() {
                return self.punctuate(fix.kana);
            }
        }
        let dictionary = Dictionary::bundled();
        let detect_whole = || self.detect_composition(&self.raw, final_input);
        let detected = || match self.preferences.english.get(&self.raw.to_ascii_lowercase()) {
            _ if self.settings.text("DetectionLevel") == "Manual" => detect_whole(),
            Some(true) => self.raw.clone(),
            Some(false) => romaji::hiragana(&self.raw, final_input),
            None => detect_whole(),
        };
        let text = crate::protection::transform(&self.raw, |part| {
            if part == self.raw {
                detected()
            } else if self.settings.text("DetectionLevel") == "Balanced"
                && !Dictionary::uses_scored(self.settings.boolean("ScoredSegmentation"))
            {
                dictionary.mixed(part, final_input)
            } else {
                self.detect_composition(part, final_input)
            }
        });
        let text = self
            .edited_units
            .as_ref()
            .filter(|u| u.raw() == self.raw)
            .filter(|_| !Dictionary::uses_scored(self.settings.boolean("ScoredSegmentation")))
            .map_or(text.clone(), |u| u.render(&text, final_input));
        self.punctuate(text)
    }
    fn language_suggestion(&self) -> Option<String> {
        (self.settings.text("DetectionLevel") == "Manual"
            && self.explicit.is_none()
            && !self.is_converting()
            && !self.uses_kana()
            && Dictionary::bundled().is_english(&self.raw)
            && self.display(false) != self.raw)
            .then(|| self.raw.clone())
    }
    /// Auto-mode Space commits a final English segment, including mixed text.
    pub fn ends_in_english(&self) -> bool {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        self.function_key == 0
            && self.explicit.is_none()
            && !self.uses_kana()
            && Dictionary::bundled().rendered_english_tail(
                &self.detect_composition(&self.raw, true),
                self.settings.text("DetectionLevel"),
            )
    }
    fn english_at_word_end(&self) -> bool {
        if self.settings.text("DetectionLevel") == "Manual"
            || self.raw.len() < 2
            || !self.raw.bytes().all(|c| c.is_ascii_alphabetic())
            || !Dictionary::bundled().known_english(&self.raw)
        {
            return false;
        }
        let analysis = romaji::analyze(&self.raw.to_ascii_lowercase(), false);
        analysis.valid
            && !analysis.partial.is_empty()
            && !["n", "nn"].contains(&analysis.partial.as_str())
    }
    fn punctuate(&self, text: String) -> String {
        self.punctuate_with_context(text, Dictionary::bundled().english_context(&self.before))
    }
    fn punctuate_with_context(&self, text: String, mut english: bool) -> String {
        let (comma, period) = match self.settings.text("Punctuation") {
            "FullWidthCommaPeriod" => ('，', '．'),
            "FullWidthCommaKuten" => ('，', '。'),
            "ToutenFullWidthPeriod" => ('、', '．'),
            _ => ('、', '。'),
        };
        crate::protection::transform(&text, |part| {
            part.chars()
                .map(|c| {
                    let latin_punctuation =
                        english && !self.settings.boolean("JapanesePunctuationAfterEnglish");
                    let output = match c {
                        '、' => {
                            if latin_punctuation {
                                ','
                            } else {
                                comma
                            }
                        }
                        '。' => {
                            if latin_punctuation {
                                '.'
                            } else {
                                period
                            }
                        }
                        '!' if !latin_punctuation => '！',
                        '?' if !latin_punctuation => '？',
                        '~' if !latin_punctuation => '～',
                        '\\' => '￥',
                        '/' if self.settings.boolean("SlashAsMiddleDot") => '・',
                        _ => c,
                    };
                    if c.is_ascii_alphabetic() {
                        english = true;
                    } else if crate::linguistics::kana(c) || crate::linguistics::kanji(c) {
                        english = false;
                    }
                    output
                })
                .collect()
        })
    }
    fn preview_candidates(&self) -> Option<Vec<String>> {
        (!self.is_converting()
            && self.function_key == 0
            && self.explicit.is_none()
            && !self.uses_kana()
            && self.raw.eq_ignore_ascii_case("go"))
        .then(|| vec![self.raw.clone(), "ご".into()])
    }
    pub fn result(&self, consumed: bool) -> SessionResult {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        let preview = self.preview_candidates();
        let language_suggestion = self.language_suggestion();
        SessionResult {
            consumed,
            selection_expect: None,
            commits: vec![],
            view: if self.raw.is_empty() {
                None
            } else {
                Some(View {
                    text: self.display(false),
                    converting: self.is_converting(),
                    selected_index: preview
                        .as_ref()
                        .and_then(|values| {
                            values
                                .iter()
                                .position(|value| value == &self.display(false))
                        })
                        .unwrap_or(self.selected),
                    selected_clause: self.selected_clause,
                    hint: language_suggestion
                        .as_ref()
                        .map(|s| format!("Tab → {s}"))
                        .unwrap_or_default(),
                    clauses: self.clauses.iter().map(|c| c.text().to_owned()).collect(),
                    candidates: preview.clone().unwrap_or_else(|| self.candidates.clone()),
                    meaning: if self.settings.boolean("ShowCandidateMeanings")
                        && self.is_converting()
                    {
                        let text = self
                            .candidates
                            .get(self.selected)
                            .cloned()
                            .unwrap_or_else(|| self.display(false));
                        let reading = self
                            .clauses
                            .get(self.selected_clause)
                            .map(|c| c.reading.clone())
                            .unwrap_or_else(|| self.reading());
                        Linguistics::bundled().meaning(&text, &reading)
                    } else {
                        None
                    },
                    suggestion: if let Some(suggestion) = language_suggestion {
                        Some(suggestion)
                    } else if self.settings.boolean("CorrectTypos") && !self.is_converting() {
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
        self.forget_direct_input();
        self.sigil = Default::default();
        self.correctable.clear();
        self.space_started_conversion = false;
        self.chosen_conversion = false;
        self.last_commit = None;
        self.reconversion_origin = None;
        self.composition_kana = false;
        self.context_is_fresh = false;
        self.raw.clear();
        self.edited_units = None;
        self.typed_keys.clear();
        self.candidates.clear();
        self.clauses.clear();
        self.live_clauses.clear();
        self.selected_clause = 0;
        self.explicit = None;
        self.selected = 0;
        self.selected_prediction = -1;
        self.translation_candidates.clear();
        self.function_key = 0;
        self.case_cycle = 0;
        self.kana_text.clear();
    }
    /// A host such as IBus has already committed its displayed preedit on
    /// focus loss. Record that exact text, without finalizing pending letters,
    /// accepting an uncommitted prediction, or changing older document text.
    pub fn commit_preedit(&mut self) -> SessionResult {
        self.explicit = Some(self.display(false));
        self.selected_prediction = -1;
        let can_delete = std::mem::replace(&mut self.can_delete, false);
        let result = self.commit();
        self.can_delete = can_delete;
        result
    }
    /// Preserve the legacy macOS C ABI's external-scalar boundary. Combining
    /// marks attach to committed text; the OS receives the original event once.
    pub fn protects_tail(&self) -> bool {
        !self.is_converting()
            && self.function_key == 0
            && !self.uses_kana()
            && crate::protection::scan(&self.raw)
                .last()
                .is_some_and(|span| span.end == self.raw.len())
    }
    pub fn commit_external(&mut self, latin: bool, preserve_text: bool) -> SessionResult {
        let raw_latin = latin
            && !self.is_converting()
            && self.reconversion_origin.is_none()
            && !self.uses_kana()
            && matches!(self.function_key, 0 | 0x79)
            && !self.raw.is_empty()
            && self.raw.bytes().all(|b| b.is_ascii_alphabetic());
        let mut result = if raw_latin {
            let text = self.raw.clone();
            self.reset();
            SessionResult {
                consumed: false,
                commits: vec![TextEdit {
                    delete_before: 0,
                    text,
                    expect: None,
                }],
                view: None,
                selection_expect: None,
            }
        } else {
            self.commit_with_options(
                !preserve_text && self.settings.boolean("SpaceAroundEnglish"),
                !preserve_text,
            )
        };
        self.before.clear();
        self.after.clear();
        self.correctable.clear();
        self.last_commit = None;
        self.sigil = Default::default();
        result.consumed = false;
        result
    }
    pub fn commit(&mut self) -> SessionResult {
        self.commit_with_spacing(self.settings.boolean("SpaceAroundEnglish"))
    }
    fn commit_with_spacing(&mut self, space_around: bool) -> SessionResult {
        self.commit_with_options(space_around, true)
    }
    fn commit_with_options(&mut self, space_around: bool, format_phrases: bool) -> SessionResult {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        let mut text = self.display(true);
        if self.selected_prediction >= 0 {
            if let Some(prediction) = self.predictions().get(self.selected_prediction as usize) {
                text = prediction.clone();
            }
        }
        let reading = self.reading();
        self.record_engine_learning();
        if !self.private
            && !self.no_learning
            && self.settings.boolean("LearningEnabled")
            && !self.raw.is_empty()
        {
            let dynamic = crate::time_candidates::is_dynamic_choice(&reading, &text)
                || self
                    .clauses
                    .iter()
                    .chain(&self.live_clauses)
                    .any(|c| crate::time_candidates::is_dynamic_choice(&c.reading, c.text()));
            let mut changes = if dynamic {
                vec![]
            } else {
                vec![Change::Phrase {
                    reading: reading.clone(),
                    text: text.clone(),
                }]
            };
            changes.extend(self.clause_language_changes());
            changes.extend(self.clause_history_changes());
            if self.is_converting() && self.clauses.is_empty() {
                let clause = self.single_conversion_clause(&reading, &text);
                if let Some(change) = Self::clause_language_change(&clause) {
                    changes.push(change);
                }
                if let Some(change) = Self::clause_history_change(&clause) {
                    changes.push(change);
                }
            }
            if self.function_key != 0 && !self.is_converting() {
                changes.push(Change::Language {
                    raw: self.raw.clone(),
                    english: self.function_key >= 0x78,
                    explicit: true,
                });
            }
            self.learn(changes);
        }
        if format_phrases && self.automatic_english_spacing && self.function_key == 0 {
            text = crate::phrase_spacing::format(&text);
        }
        if space_around {
            text = space_english(&text, &self.before, &self.after);
        }
        let english =
            !text.trim().is_empty() && text.trim().bytes().all(|b| b.is_ascii_alphabetic());
        let after_english = Dictionary::bundled().english_context(&self.before);
        let chosen = self.function_key != 0
            || self.chosen_conversion
            || self.selected_prediction >= 0
            || self.is_converting() && self.selected != 0
            || self.reconversion_origin.is_some();
        let correction = if self.can_delete
            && !self.private
            && !self.direct
            && !self.code_english()
            && self.settings.boolean("AutoCorrectAfterCommit")
            && self.settings.text("DetectionLevel") != "Manual"
            && !chosen
        {
            crate::commit_correction::replacement(
                &self.correctable,
                &self.raw,
                english,
                &self.before,
                &self.preferences,
            )
        } else {
            None
        };
        let mut records = std::mem::take(&mut self.correctable);
        if let Some((original, replacement)) = &correction {
            self.before.truncate(self.before.len() - original.len());
            self.before.push_str(replacement);
            records.clear();
        }
        let dictionary = Dictionary::bundled();
        if !self.private
            && !chosen
            && self.settings.text("DetectionLevel") != "Manual"
            && self.reconversion_origin.is_none()
            && dictionary.is_english(&self.raw)
            && dictionary.is_japanese(&self.raw)
        {
            if records.last().is_some_and(|r| !r.space) {
                records.clear();
            }
            records.push(crate::commit_correction::Record {
                raw: self.raw.clone(),
                text: text.clone(),
                english,
                space: self.space_started_conversion,
                after_english,
            });
            if records.len() > 4 {
                records.remove(0);
            }
        } else if !text.is_empty() {
            records.clear();
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
        let raw = self.raw.clone();
        let keys = self.typed_keys.clone();
        let units = self.edited_units.take();
        let romaji = self.custom_romaji.clone();
        self.reset();
        self.correctable = records;
        if !self.private {
            self.sigil.append(&text);
        }
        if !self.private && !text.is_empty() {
            self.last_commit = Some(CommittedInput {
                raw,
                text: text.clone(),
                keys,
                units,
                romaji,
            });
        }
        SessionResult {
            consumed: !text.is_empty(),
            commits: if text.is_empty() {
                vec![]
            } else {
                vec![TextEdit {
                    delete_before: correction
                        .as_ref()
                        .map_or(0, |(original, _)| original.encode_utf16().count()),
                    text: correction
                        .as_ref()
                        .map_or(text.clone(), |(_, replacement)| replacement.clone() + &text),
                    expect: correction.map(|(original, _)| original),
                }]
            },
            view: None,
            selection_expect: None,
        }
    }
    /// Start reconversion without deleting text. Hosts must verify the selected
    /// text against selection_expect before replacing it with the preedit.
    pub fn reconvert_selection(
        &mut self,
        text: &str,
        supplied_reading: Option<&str>,
    ) -> SessionResult {
        if !self.raw.is_empty()
            || self.private
            || !self.settings.boolean("Enabled")
            || text.is_empty()
            || text.chars().count() > 1024
        {
            return self.result(false);
        }
        let reading = supplied_reading
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                self.state
                    .phrases
                    .iter()
                    .filter(|p| p.text == text)
                    .max_by_key(|p| (p.used, p.count))
                    .map(|p| p.reading.clone())
            })
            .or_else(|| {
                self.preferences
                    .user_dictionary
                    .iter()
                    .find(|(_, words)| words.iter().any(|w| w == text))
                    .map(|(r, _)| r.clone())
            })
            .or_else(|| {
                self.preferences
                    .selected
                    .iter()
                    .find(|(_, word)| word.as_str() == text)
                    .map(|(r, _)| r.clone())
            })
            .or_else(|| Dictionary::bundled().reading_for(text))
            .unwrap_or_else(|| romaji::hiragana(&crate::user_dictionary::hiragana(text), true));
        let reading = crate::user_dictionary::hiragana(&reading);
        let literal =
            supplied_reading == Some(text) && self.romaji_reconversion_parts(text).is_some();
        if reading.chars().count() > 1024
            || !literal && reading.chars().any(|c| !crate::linguistics::kana(c))
        {
            return self.result(false);
        }
        self.forget_direct_input();
        self.correctable.clear();
        self.reconversion_origin = Some(text.to_owned());
        self.raw = reading.clone();
        self.typed_keys = reading.chars().map(|c| (0, c, false)).collect();
        self.kana_text = reading;
        self.begin_conversion(vec![text.to_owned()]);
        if let Some(index) = self.candidates.iter().position(|c| c == text) {
            self.selected = index;
        }
        self.reconversion_origin = Some(text.to_owned());
        let mut result = self.result(true);
        result.selection_expect = Some(text.to_owned());
        result
    }
    /// Restore the last committed input as a composition. The host must recheck
    /// `expect` immediately before deleting; `delete_before` counts UTF-16 units.
    pub fn undo_commit(&mut self) -> SessionResult {
        if !self.can_delete || self.private || !self.raw.is_empty() {
            return self.result(false);
        }
        let Some(CommittedInput {
            raw,
            text,
            keys,
            units,
            romaji,
        }) = self.last_commit.take()
        else {
            return self.result(false);
        };
        self.correctable.clear();
        if !self.before.ends_with(&text) {
            return self.result(false);
        }
        self.before.truncate(self.before.len() - text.len());
        self.raw = raw;
        self.typed_keys = keys;
        self.edited_units = units;
        self.custom_romaji = romaji;
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        self.refresh_kana();
        let mut result = self.result(true);
        result.commits.push(TextEdit {
            delete_before: text.encode_utf16().count(),
            text: String::new(),
            expect: Some(text),
        });
        result
    }
    /// macOS candidate clicks select a composition; Enter commits afterward.
    pub fn select_composing(&mut self, index: usize) -> SessionResult {
        if let Some(preview) = self.preview_candidates() {
            if index >= preview.len() {
                return self.result(false);
            }
            self.candidates = preview;
            let mut clause = Clause::new("ご".into(), self.candidates.clone());
            clause.raw = Some(self.raw.clone());
            self.clauses = vec![clause];
            self.live_clauses.clear();
        }
        if index >= self.candidates.len() {
            return self.result(false);
        }
        self.chosen_conversion = true;
        self.selected = index;
        if let Some(clause) = self.clauses.get_mut(self.selected_clause) {
            clause.selected = index;
            clause.changed = true;
        }
        self.result(true)
    }
    pub fn select(&mut self, index: usize) -> SessionResult {
        if self.preview_candidates().is_some() {
            let result = self.select_composing(index);
            return if result.consumed {
                self.commit()
            } else {
                result
            };
        }
        if index >= self.candidates.len() {
            return self.result(false);
        }
        self.chosen_conversion = true;
        self.selected = index;
        if let Some(clause) = self.clauses.get_mut(self.selected_clause) {
            clause.selected = index;
            clause.changed = true;
            if self.clauses.len() > 1 {
                return self.result(true);
            }
        }
        self.commit()
    }
    fn select_number(&mut self, row: usize) -> Option<SessionResult> {
        let index = self.selected / 9 * 9 + row;
        if index >= self.candidates.len() {
            return None;
        }
        self.chosen_conversion = true;
        self.selected = index;
        if let Some(clause) = self.clauses.get_mut(self.selected_clause) {
            clause.selected = index;
            clause.changed = true;
            if self.selected_clause + 1 < self.clauses.len() {
                self.selected_clause += 1;
                self.sync_clause();
                return Some(self.result(true));
            }
        }
        Some(self.commit())
    }
    pub fn begin_conversion(&mut self, external: Vec<String>) {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        if self.raw.is_empty() || self.is_converting() {
            return;
        }
        if !self.live_clauses.is_empty() {
            let clauses = std::mem::take(&mut self.live_clauses);
            self.begin_clauses(clauses);
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
        if self.external_converter_only {
            // The macOS frontend delegates conversion to its registered azooKey
            // callbacks. With no converter result, preserve the complete reading.
            candidates.push(reading.clone());
        } else {
            candidates.extend(Dictionary::bundled().candidates(&self.display(true)));
            candidates.extend(Dictionary::bundled().candidates(&reading));
            candidates.extend(Linguistics::bundled().words(&reading));
            if let Some(context) =
                Linguistics::bundled().context(&reading, &format!("{}{}", self.before, self.after))
            {
                candidates.insert(0, context);
            }
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
        let emoji = Dictionary::bundled().emoji(&reading);
        unique.retain(|candidate| !emoji.contains(candidate));
        unique.extend(emoji);
        crate::math_candidates::augment(&reading, &self.before, &mut unique);
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
    /// Request live conversion only for settled Japanese input. English islands
    /// and protected tokens retain their original text.
    pub fn live_reading(&self) -> Option<String> {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        if !self.settings.boolean("LiveConversion")
            || self.is_converting()
            || self.explicit.is_some()
            || self.direct
            || self.private
            || self.code_english()
            || !self.settings.boolean("Enabled")
            || self.raw.is_empty()
        {
            return None;
        }
        let reading = self.reading();
        if self.external_converter_only {
            let pending = self.mixed_reading(false);
            return (reading.encode_utf16().count() >= 4
                && pending == reading
                && reading.chars().any(crate::linguistics::kana))
            .then_some(reading);
        }
        if reading.encode_utf16().count() < 4
            || romaji::hiragana(&self.raw, false) != reading
            || Dictionary::bundled()
                .mixed(&self.raw, false)
                .chars()
                .any(|c| c.is_ascii_alphabetic())
            || reading.chars().any(|c| !crate::linguistics::kana(c))
        {
            return None;
        }
        Some(reading)
    }
    pub fn update_live_conversion(&mut self, mut clauses: Vec<Clause>) -> bool {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        let Some(reading) = self.live_reading() else {
            return false;
        };
        if clauses.is_empty()
            || clauses.iter().any(|c| {
                c.reading.is_empty() || c.candidates.is_empty() || c.selected >= c.candidates.len()
            })
            || clauses
                .iter()
                .map(|c| c.reading.as_str())
                .collect::<String>()
                != reading
        {
            return false;
        }
        self.rank_clauses(&mut clauses);
        self.live_clauses = clauses;
        true
    }
    /// Accept segments only when they cover the entire current reading.
    pub fn begin_clauses(&mut self, mut clauses: Vec<Clause>) -> bool {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        if self.raw.is_empty()
            || clauses.is_empty()
            || clauses.iter().any(|c| {
                c.reading.is_empty() || c.candidates.is_empty() || c.selected >= c.candidates.len()
            })
            || clauses
                .iter()
                .map(|c| c.reading.as_str())
                .collect::<String>()
                != self.conversion_reading()
        {
            return false;
        }
        clauses = crate::time_candidates::join(clauses);
        let original_inputs = self.clause_original_inputs(&clauses);
        self.translation_candidates.clear();
        let mut preceding = self.before.clone();
        let mut previous_english = false;
        for (clause, original_input) in clauses.iter_mut().zip(original_inputs) {
            clause.raw = original_input;
            if clause.english {
                if clause.kana.is_some() {
                    clause.kana = Some(romaji::hiragana(&clause.reading, true));
                }
                preceding.push_str(clause.text());
                previous_english = true;
                continue;
            }
            let mut values = vec![clause.text().to_owned()];
            values.extend(
                self.preferences
                    .user_dictionary
                    .get(&clause.reading)
                    .cloned()
                    .unwrap_or_default(),
            );
            values.extend(clause.candidates.clone());
            if !self.external_converter_only {
                values.extend(Dictionary::bundled().candidates(&clause.reading));
                // MeaningDictionary supplies annotations in the original, not
                // extra conversion candidates. Adding its reverse readings
                // changes both candidate numbering and the final-page size.
                if let Some(context) = Linguistics::bundled()
                    .context(&clause.reading, &format!("{}{}", self.before, self.after))
                {
                    values.insert(0, context);
                }
            } else if !values.contains(&clause.reading) {
                values.push(clause.reading.clone());
            }
            let translations = if self.settings.boolean("TranslationCandidates") {
                let translations = Linguistics::bundled().translations(
                    values.first().map(String::as_str).unwrap_or(""),
                    &clause.reading,
                );
                self.translation_candidates.extend(translations.clone());
                values.extend(translations.clone());
                translations
            } else {
                Vec::new()
            };
            crate::math_candidates::augment(&clause.reading, &preceding, &mut values);
            // AddRawCandidates in the original controller places the typed
            // spelling and its fullwidth form after the Japanese alternatives;
            // MoveEmojiLast then moves emoji behind those spellings.
            if let Some(raw) = clause
                .raw
                .as_ref()
                .filter(|raw| raw.chars().any(|c| ('!'..='~').contains(&c)))
            {
                values.extend([raw.clone(), crate::kana::fullwidth(raw)]);
            }
            let emoji = Dictionary::bundled().emoji(&clause.reading);
            values.retain(|candidate| !emoji.contains(candidate));
            values.extend(emoji);
            let has_preference = self.preferences.selected.contains_key(&clause.reading)
                || !self.external_converter_only
                    && Linguistics::bundled()
                        .context(&clause.reading, &format!("{}{}", self.before, self.after))
                        .is_some();
            let particle = [
                "は", "が", "を", "に", "で", "と", "も", "へ", "の", "や", "か", "から", "まで",
                "より",
            ]
            .contains(&clause.reading.as_str());
            let suru = previous_english
                && [
                    "する",
                    "すれ",
                    "した",
                    "して",
                    "しま",
                    "しな",
                    "しよ",
                    "しと",
                    "しちゃ",
                    "しろ",
                    "され",
                    "させ",
                    "せず",
                ]
                .iter()
                .any(|stem| clause.reading.starts_with(stem))
                && values
                    .first()
                    .and_then(|text| text.chars().next())
                    .is_some_and(|ch| !crate::linguistics::kana(ch));
            if !has_preference && particle || suru {
                values.insert(0, clause.reading.clone());
            }
            let raw = clause.raw.take();
            *clause = Clause::new(clause.reading.clone(), values);
            clause.raw = raw;
            clause.translations = translations;
            preceding.push_str(clause.text());
            previous_english = false;
        }
        self.rank_clauses(&mut clauses);
        self.live_clauses.clear();
        self.clauses = clauses;
        self.selected_clause = 0;
        self.explicit = None;
        self.selected_prediction = -1;
        self.sync_clause();
        true
    }
    fn rank_clauses(&self, clauses: &mut [Clause]) {
        for clause in clauses {
            if clause.english {
                continue;
            }
            if let Some(preferred) = self.preferences.selected.get(&clause.reading) {
                if let Some(index) = clause.candidates.iter().position(|c| c == preferred) {
                    clause.selected = index;
                }
            }
        }
    }
    fn sync_clause(&mut self) {
        if let Some(clause) = self.clauses.get(self.selected_clause) {
            self.candidates = clause.candidates.clone();
            self.selected = clause.selected;
        }
    }
    /// Snapshot converter readings before a native host releases its registry
    /// lock to obtain fresh candidates. These are conversion clauses only.
    pub fn clause_readings(&self) -> Vec<String> {
        if self.clauses.is_empty() && self.is_converting() {
            vec![self.reading()]
        } else {
            self.clauses.iter().map(|c| c.reading.clone()).collect()
        }
    }
    /// Apply converter replies only to the unchanged composition snapshot.
    /// Callbacks run outside the native registry lock and may return no result.
    pub fn refresh_clause_candidates(
        &mut self,
        expected: &[String],
        external: Vec<(usize, Vec<String>)>,
    ) -> bool {
        if self.clause_readings() != expected || !self.is_converting() {
            return false;
        }
        for (index, candidates) in external {
            if candidates.is_empty() || index >= self.clauses.len() || self.clauses[index].english {
                continue;
            }
            let reading = self.clauses[index].reading.clone();
            self.clauses[index] = self.local_clause_with_external(reading, candidates);
        }
        self.sync_clause();
        true
    }
    fn local_clause(&self, reading: String) -> Clause {
        self.local_clause_with_external(reading, vec![])
    }
    fn local_clause_with_external(&self, reading: String, external: Vec<String>) -> Clause {
        let mut candidates = self
            .preferences
            .user_dictionary
            .get(&reading)
            .cloned()
            .unwrap_or_default();
        candidates.extend(external);
        if !self.external_converter_only {
            candidates.extend(Dictionary::bundled().candidates(&reading));
            candidates.extend(Linguistics::bundled().words(&reading));
        }
        if let Some(learned) = self.preferences.selected.get(&reading) {
            candidates.insert(0, learned.clone());
        }
        Clause::new(reading, candidates)
    }
    fn resize_clause(&mut self, grow: bool) {
        let index = self.selected_clause;
        let Some(current) = self.clauses.get(index) else {
            return;
        };
        let as_japanese = |clause: &Clause| -> Option<String> {
            if clause.english {
                clause
                    .kana
                    .as_ref()
                    .filter(|kana| kana.chars().any(crate::linguistics::kana))
                    .cloned()
            } else {
                Some(clause.reading.clone())
            }
        };
        let Some(mut reading) = as_japanese(current) else {
            return;
        };
        // Validate the move before replacing an English clause. A failed
        // resize must preserve its display and selected candidate verbatim.
        if grow {
            let Some(mut next) = self.clauses.get(index + 1).and_then(as_japanese) else {
                return;
            };
            let c = next.remove(0);
            reading.push(c);
            if next.is_empty() {
                self.clauses.remove(index + 1);
            } else {
                self.clauses[index + 1] = self.local_clause(next);
            }
        } else {
            if reading.chars().count() <= 1 {
                return;
            }
            let c = reading.pop().unwrap();
            // Shrinking before English inserts a new Japanese clause instead
            // of absorbing/reinterpreting that untouched English clause.
            let next = if self.clauses.get(index + 1).is_some_and(|c| !c.english) {
                format!("{c}{}", self.clauses.remove(index + 1).reading)
            } else {
                c.to_string()
            };
            let clause = self.local_clause(next);
            self.clauses.insert(index + 1, clause);
        }
        self.clauses[index] = self.local_clause(reading);
        self.sync_clause();
    }
    /// Virtual keys follow the existing native ABI; modifiers: shift=1, ctrl=2,
    /// alt=4, command=8. Platform shortcuts commit first and pass through.
    pub fn key(&mut self, vk: i32, ch: i32, modifiers: i32) -> SessionResult {
        let now_ms = self.clock.elapsed().as_millis().min(u64::MAX as u128) as u64;
        self.key_at(vk, ch, modifiers, now_ms)
    }
    /// Monotonic milliseconds, for hosts with an event clock and deterministic
    /// replay tests. Use one clock consistently for a session.
    pub fn key_at(&mut self, vk: i32, ch: i32, modifiers: i32, now_ms: u64) -> SessionResult {
        let _romaji = romaji::Scope::new(self.custom_romaji.clone());
        if ch < 0 || char::from_u32(ch as u32).is_none() {
            return self.result(false);
        }
        self.key_time_ms = now_ms;
        if self.direct && modifiers & 15 != 0 && !self.direct_pending.is_empty() {
            self.direct_pending.clear();
            self.direct_last_key_ms = None;
            self.direct_english_word = true;
        }
        let timeout_edit = self.direct_timeout(now_ms);
        let c = char::from_u32(ch as u32).unwrap();
        if self.private {
            self.sigil = Default::default();
        }
        if self.raw.is_empty()
            && !self.private
            && self.settings.boolean("Enabled")
            && self.settings.boolean("SigilWordsDirect")
            && !self.uses_kana()
            && modifiers & 14 == 0
            && self.sigil.passes(c)
        {
            self.correctable.clear();
            self.last_commit = None;
            self.forget_direct_input();
            self.sigil.passed(vk, c, modifiers);
            return self.result(false);
        }
        let mut result = self.key_inner(vk, ch, modifiers);
        if let Some(edit) = timeout_edit {
            result.commits.insert(0, edit);
        }
        if !result.consumed && !self.private {
            self.sigil.passed(vk, c, modifiers);
        }
        result
    }
    fn key_inner(&mut self, vk: i32, ch: i32, modifiers: i32) -> SessionResult {
        if let Some(result) = self.detect_direct(vk, ch, modifiers) {
            return result;
        }
        let reconverting = self.reconversion_origin.is_some();
        let code_direct = self.code_english() && !reconverting;
        if modifiers & 14 == 2
            && !self.raw.is_empty()
            && (!self.direct || reconverting)
            && !self.private
            && !code_direct
            && self.settings.boolean("Enabled")
        {
            let mapped = match vk {
                0x48 => Some(8),
                0x55 => Some(0x75),
                0x49 => Some(0x76),
                0x4f => Some(0x79),
                0x50 => Some(0x78),
                _ => None,
            };
            if let Some(mapped) = mapped {
                return self.key(mapped, 0, modifiers & 1);
            }
        }
        if modifiers & 14 != 0
            || self.direct && !reconverting
            || self.private
            || code_direct
            || !self.settings.boolean("Enabled")
        {
            self.correctable.clear();
            if self.raw.is_empty() {
                return self.result(false);
            }
            let mut result = self.commit();
            self.correctable.clear();
            result.consumed = false;
            return result;
        }
        if self.is_converting()
            && modifiers & 1 == 0
            && (matches!(vk, 0x31..=0x39) && !self.uses_kana() || matches!(vk, 0x61..=0x69))
        {
            let row = if vk >= 0x61 { vk - 0x61 } else { vk - 0x31 };
            if let Some(result) = self.select_number(row as usize) {
                return result;
            }
        }
        match vk {
            0x20 if modifiers & 1 != 0 && !self.raw.is_empty() => {
                if self.is_converting() {
                    self.cycle(-1);
                } else {
                    self.space_started_conversion = true;
                    self.begin_conversion(vec![]);
                }
                self.result(true)
            }
            0x20 if modifiers & 1 != 0 => {
                let mut result = self.commit();
                self.correctable.clear();
                result.commits.push(TextEdit {
                    delete_before: 0,
                    text: "　".into(),
                    expect: None,
                });
                self.before.push('　');
                result.consumed = true;
                result
            }
            0x20 if self.raw.is_empty() => {
                self.correctable.clear();
                self.result(false)
            }
            0x10..=0x12 | 0xa0..=0xa5 | 0x5b..=0x5c => self.result(false),
            0x1b => {
                if let Some(original) = self.reconversion_origin.take() {
                    self.reset();
                    return SessionResult {
                        consumed: true,
                        commits: vec![TextEdit {
                            delete_before: 0,
                            text: original,
                            expect: None,
                        }],
                        view: None,
                        selection_expect: None,
                    };
                }
                if self.raw.is_empty() {
                    return self.result(false);
                }
                if self.is_converting() {
                    self.candidates.clear();
                    self.clauses.clear();
                    self.live_clauses.clear();
                    self.selected_clause = 0;
                    self.explicit = None;
                } else {
                    self.reset();
                }
                self.result(true)
            }
            0x08 => {
                if self.raw.is_empty() {
                    self.correctable.clear();
                    return self.result(false);
                }
                let displayed = self.display(false);
                let english_tail = matches!(self.function_key, 0x78 | 0x79)
                    || self.function_key == 0
                        && displayed
                            .chars()
                            .next_back()
                            .is_some_and(|c| c.is_ascii_alphabetic());
                let previous_keys = self.raw.chars().count();
                if self.uses_kana() || self.protects_tail() {
                    self.raw.pop();
                    self.edited_units = None;
                } else {
                    if self
                        .edited_units
                        .as_ref()
                        .is_none_or(|u| u.raw() != self.raw)
                    {
                        self.edited_units = Some(crate::composition_units::Units::capture(
                            &self.raw, &displayed,
                        ));
                    }
                    let units = self.edited_units.as_mut().expect("captured composition");
                    units.remove_last(english_tail, displayed == self.raw);
                    self.raw = units.raw().to_owned();
                }
                self.candidates.clear();
                self.clauses.clear();
                self.live_clauses.clear();
                self.selected_clause = 0;
                self.explicit = None;
                self.selected = 0;
                self.selected_prediction = -1;
                self.function_key = 0;
                self.case_cycle = 0;
                let removed_keys = previous_keys - self.raw.chars().count();
                self.typed_keys
                    .truncate(self.typed_keys.len().saturating_sub(removed_keys));
                self.refresh_kana();
                self.result(true)
            }
            0x0d => {
                if self.raw.is_empty() {
                    self.correctable.clear();
                }
                self.commit()
            }
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
            0x20 | 0x09 if self.protects_tail() => {
                let suffix = if vk == 0x20 { " " } else { "" };
                self.explicit = Some(self.display(true) + suffix);
                let mut result = self.commit_with_spacing(false);
                result.consumed = vk == 0x20;
                result
            }
            0x09 if !self.is_converting()
                && self.function_key == 0
                && !crate::protection::scan(&self.raw).is_empty() =>
            {
                let mut result = self.commit();
                result.consumed = false;
                result
            }
            0x09 if !self.raw.is_empty() && !self.is_converting() => {
                if let Some(suggestion) = self.language_suggestion() {
                    self.explicit = Some(suggestion);
                    self.function_key = 0x79;
                    self.case_cycle = 0;
                    self.selected_prediction = -1;
                    return self.result(true);
                }
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
                if self.space_commits_alphanumeric() {
                    self.space_started_conversion = true;
                    self.explicit = Some(self.display(true) + " ");
                    return self.commit();
                }
                if !self.is_converting()
                    && !self.settings.boolean("SeparateKeyRoles")
                    && self.ends_in_english()
                {
                    self.space_started_conversion = true;
                    self.explicit = Some(self.display(true) + " ");
                    return self.commit();
                }
                if !self.is_converting()
                    && !self.settings.boolean("SeparateKeyRoles")
                    && (self.display(true) == self.raw
                        || self.english_at_word_end()
                        || self.before.is_empty()
                            && !["Manual", "Conservative"]
                                .contains(&self.settings.text("DetectionLevel"))
                            && Dictionary::bundled().is_common_english(&self.raw)
                            && (!Dictionary::bundled().is_japanese(&self.raw)
                                || self.raw.starts_with(|c: char| c.is_ascii_uppercase())
                                || [
                                    "no", "i", "we", "you", "they", "he", "she", "it", "the",
                                    "this", "that", "please", "make", "do", "can", "will", "let",
                                    "would", "should", "have", "has", "get", "take", "look",
                                    "open", "run",
                                ]
                                .contains(&self.raw.to_ascii_lowercase().as_str())))
                    && (Dictionary::bundled().known_english(&self.raw)
                        || Dictionary::bundled().english_context(&self.before)
                            && self.raw.bytes().all(|c| c.is_ascii_alphabetic()))
                {
                    self.space_started_conversion = true;
                    self.explicit = Some(self.raw.clone() + " ");
                    return self.commit();
                }
                if self.is_converting() {
                    self.cycle(if modifiers & 1 != 0 { -1 } else { 1 });
                } else {
                    self.space_started_conversion = true;
                    self.begin_conversion(vec![]);
                }
                self.result(true)
            }
            0x25 | 0x27 if self.is_converting() => {
                if self.clauses.is_empty() {
                    self.clauses.push(Clause {
                        reading: self.reading(),
                        candidates: self.candidates.clone(),
                        selected: self.selected,
                        english: false,
                        kana: None,
                        raw: if self.uses_kana() || self.reconversion_origin.is_some() {
                            None
                        } else {
                            Some(self.raw.clone())
                        },
                        changed: self.chosen_conversion,
                        translations: self.translation_candidates.clone(),
                    });
                }
                if modifiers & 1 != 0 {
                    self.resize_clause(vk == 0x27);
                } else {
                    self.selected_clause = if vk == 0x25 {
                        self.selected_clause.saturating_sub(1)
                    } else {
                        (self.selected_clause + 1).min(self.clauses.len() - 1)
                    };
                    self.sync_clause();
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
                let before = (!self.is_converting()).then(|| self.display(false));
                let same_mode = !self.is_converting() && self.function_key == vk;
                self.candidates.clear();
                self.clauses.clear();
                self.live_clauses.clear();
                self.selected_clause = 0;
                self.case_cycle = if self.function_key == vk {
                    (self.case_cycle + 1) % 3
                } else {
                    0
                };
                self.function_key = vk;
                if vk >= 0x78 {
                    let render = |case: u8| {
                        let raw = match case {
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
                        if vk == 0x78 {
                            crate::kana::fullwidth(&raw)
                        } else {
                            raw
                        }
                    };
                    if !same_mode {
                        self.case_cycle = 0;
                    }
                    let mut output = render(self.case_cycle);
                    if same_mode || before.as_deref() == Some(output.as_str()) {
                        // Try all three cases: one-letter/uppercase/numeric
                        // inputs can have identical visible representations.
                        // same_mode already advanced the cycle above.
                        for _ in 0..3 {
                            if before.as_deref() != Some(output.as_str()) {
                                break;
                            }
                            self.case_cycle = (self.case_cycle + 1) % 3;
                            output = render(self.case_cycle);
                        }
                    }
                    self.explicit = Some(output);
                    return self.result(true);
                }
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
                self.explicit = Some(
                    if (self.uses_kana()
                        || self
                            .edited_units
                            .as_ref()
                            .is_some_and(|u| u.raw() == self.raw))
                        && vk <= 0x77
                    {
                        match vk {
                            0x75 => reading,
                            0x76 => romaji::katakana(&reading),
                            _ => crate::kana::halfwidth(&romaji::katakana(&reading)),
                        }
                    } else if vk == 0x78 {
                        crate::kana::fullwidth(&raw)
                    } else if vk == 0x79 {
                        raw.clone()
                    } else {
                        crate::protection::transform(&raw, |part| match vk {
                            0x75 => romaji::hiragana(part, true),
                            0x76 => romaji::katakana(&romaji::hiragana(part, true)),
                            0x77 => crate::kana::halfwidth(&romaji::katakana(&romaji::hiragana(
                                part, true,
                            ))),
                            0x78 => crate::kana::fullwidth(part),
                            _ => part.into(),
                        })
                    },
                );
                self.result(true)
            }
            _ if ch >= 0x20 && ch != 0x7f => {
                // Retain the old commit when a new character starts after conversion.
                let commits =
                    if self.is_converting() && !self.settings.boolean("ContinueAfterConversion") {
                        self.commit().commits
                    } else {
                        self.candidates.clear();
                        self.clauses.clear();
                        self.live_clauses.clear();
                        self.selected_clause = 0;
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
                let character = char::from_u32(ch as u32).unwrap();
                if self.raw.is_empty() && self.custom_romaji.is_some() && !self.uses_kana() {
                    self.edited_units = Some(Default::default());
                }
                if let Some(units) = self.edited_units.as_mut() {
                    if units.raw() == self.raw {
                        units.push(&character.to_string());
                    } else {
                        self.edited_units = None;
                    }
                }
                self.raw.push(character);
                self.typed_keys
                    .push((vk, char::from_u32(ch as u32).unwrap(), modifiers & 1 != 0));
                self.refresh_kana();
                let mut result = self.result(true);
                result.commits = commits;
                result
            }
            _ => {
                self.correctable.clear();
                let mut result = self.commit();
                self.correctable.clear();
                result.consumed = false;
                result
            }
        }
    }
    fn cycle(&mut self, direction: i32) {
        self.chosen_conversion = true;
        self.selected =
            (self.selected as i32 + direction).rem_euclid(self.candidates.len() as i32) as usize;
        if let Some(clause) = self.clauses.get_mut(self.selected_clause) {
            clause.selected = self.selected;
            clause.changed = true;
        }
    }
}

fn space_english(text: &str, before: &str, after: &str) -> String {
    let japanese = |c: char| matches!(c,'\u{3040}'..='\u{30ff}'|'\u{3400}'..='\u{4dbf}'|'\u{4e00}'..='\u{9fff}'|'\u{f900}'..='\u{faff}');
    let word_char = |c: char| c.is_ascii_alphanumeric() || "_-+#.'@".contains(c);
    let chars: Vec<_> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let previous = before.chars().next_back().unwrap_or('\0');
    let next = after.chars().next().unwrap_or('\0');
    let mut output = String::new();
    if japanese(chars[0]) && previous.is_ascii_alphabetic() {
        output.push(' ');
    }
    let mut index = 0;
    while index < chars.len() {
        if !word_char(chars[index]) {
            output.push(chars[index]);
            index += 1;
            continue;
        }
        let mut end = index;
        while end < chars.len() && word_char(chars[end]) {
            end += 1;
        }
        let mut stop = end;
        while stop > index && ".'-@_".contains(chars[stop - 1]) {
            stop -= 1;
        }
        let word = &chars[index..stop];
        let left = if index > 0 {
            chars[index - 1]
        } else {
            previous
        };
        let right = chars.get(stop).copied().unwrap_or(next);
        let english = word.iter().any(char::is_ascii_alphabetic);
        if english && japanese(left) && !output.ends_with(' ') {
            output.push(' ');
        }
        output.extend(word);
        if english && japanese(right) {
            output.push(' ');
        }
        output.extend(&chars[stop..end]);
        index = end;
    }
    if japanese(*chars.last().unwrap()) && next.is_ascii_alphabetic() {
        output.push(' ');
    }
    output
}
