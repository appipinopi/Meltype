// SPDX-License-Identifier: GPL-3.0-or-later
//! CompositionController.LearnConversion notifications, separate from history.
use super::*;
use crate::mozc::LearningClause;
#[derive(Debug)]
pub struct Batch {
    pub context: String,
    pub clauses: Vec<LearningClause>,
}
impl Session {
    /// Only an embedding with an installed learner should retain events.
    pub fn collect_engine_learning(&mut self, enabled: bool) {
        if enabled {
            self.engine_learning.get_or_insert_with(Vec::new);
        } else {
            self.engine_learning = None;
        }
    }
    pub fn take_engine_learning(&mut self) -> Vec<Batch> {
        self.engine_learning
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }
    pub(super) fn record_engine_learning(&mut self) {
        if self.engine_learning.is_none()
            || self.private
            || self.no_learning
            || self.v130.esc_reverted
            || !self.settings.boolean("LearningEnabled")
            || !self.is_converting()
            || self.explicit.is_some()
            || self.selected_prediction >= 0
        {
            return;
        }
        let context = conversion_context(&self.before);
        let mut run = Vec::new();
        let mut batches = Vec::new();
        let mut flush = |run: &mut Vec<LearningClause>| {
            if !run.is_empty() {
                batches.push(Batch {
                    context: context.clone(),
                    clauses: std::mem::take(run),
                });
            }
        };
        if self.clauses.is_empty() {
            let clause = self.single_conversion_clause(&self.reading(), &self.display(true));
            if eligible(
                clause.english,
                &clause.reading,
                clause.text(),
                clause.raw.as_deref().unwrap_or(""),
                &clause.translations,
            ) {
                run.push(LearningClause {
                    reading: clause.reading.clone(),
                    text: clause.text().to_owned(),
                });
            }
        } else {
            for clause in &self.clauses {
                if eligible(
                    clause.english,
                    &clause.reading,
                    clause.text(),
                    clause.raw.as_deref().unwrap_or(""),
                    &clause.translations,
                ) {
                    run.push(LearningClause {
                        reading: clause.reading.clone(),
                        text: clause.text().to_owned(),
                    });
                } else {
                    flush(&mut run);
                }
            }
        }
        flush(&mut run);
        self.engine_learning.as_mut().unwrap().extend(batches);
    }
}
fn eligible(english: bool, reading: &str, text: &str, raw: &str, translations: &[String]) -> bool {
    !english
        && text != raw
        && !translations.iter().any(|t| t == text)
        && !reading.chars().any(|c| c.is_ascii_alphanumeric())
        && !crate::time_candidates::is_dynamic_choice(reading, text)
}
fn conversion_context(before: &str) -> String {
    // Original LanguageOf scans UTF-16 from the end: ASCII letters/digits are
    // English; non-whitespace >= U+2000 (including surrogate units) Japanese.
    let units: Vec<_> = before.encode_utf16().collect();
    let japanese = units.iter().rev().find_map(|&c| {
        if c <= 127 && (c as u8).is_ascii_alphanumeric() {
            Some(false)
        } else if c >= 0x2000 && !char::from_u32(c as u32).is_some_and(char::is_whitespace) {
            Some(true)
        } else if c >= 0x80 && !char::from_u32(c as u32).is_some_and(char::is_whitespace) {
            Some(false)
        } else {
            None
        }
    }) == Some(true);
    if !japanese {
        return String::new();
    }
    let sentence = before
        .rsplit(['。', '．', '！', '？', '\n', '\r'])
        .next()
        .unwrap_or("")
        .trim();
    let units: Vec<_> = sentence.encode_utf16().collect();
    String::from_utf16_lossy(&units[units.len().saturating_sub(10)..])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_function_keys_conversion_and_engine_learning_match() {
        let golden: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/data/upstream-engine-keys.json"))
                .unwrap();
        let cases = golden["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 50);
        for row in cases {
            let c = &row["case"];
            let raw = c["raw"].as_str().unwrap();
            // The reference enables BuiltInWordChecker. Core-only sessions
            // receive resolved spelling facts from their native embedding.
            let _spelling = (c["level"] == "Balanced").then(|| {
                crate::spelling::Scope::new(std::collections::HashMap::from([(
                    "camera".into(),
                    true,
                )]))
            });
            for structured in [false, true] {
                // The English single-clause classification is exercised by the
                // unsegmented path. Mixed/structured English generation has its
                // own conversion-reader coverage.
                if structured && raw == "HASHI" {
                    continue;
                }
                let mut s = Session {
                    no_learning: c["private"].as_bool().unwrap(),
                    ..Default::default()
                };
                for (name, value) in [
                    ("LiveConversion", false),
                    ("PredictiveCandidates", false),
                    ("LearningEnabled", c["learning"].as_bool().unwrap()),
                    (
                        "SeparateKeyRoles",
                        c["separate"].as_bool().unwrap_or(c["action"] == "convert"),
                    ),
                ] {
                    s.settings.set(name, serde_json::json!(value)).unwrap();
                }
                s.settings
                    .set(
                        "DetectionLevel",
                        serde_json::json!(c["level"].as_str().unwrap_or("Manual")),
                    )
                    .unwrap();
                s.collect_engine_learning(true);
                let mut committed = String::new();
                let mut append = |result: SessionResult| {
                    for edit in result.commits {
                        committed.push_str(&edit.text);
                    }
                };
                for ch in raw.chars() {
                    append(s.key(
                        ch.to_ascii_uppercase() as i32,
                        ch as i32,
                        i32::from(ch.is_ascii_uppercase()),
                    ));
                }
                let force = c["force"].as_i64().unwrap() as i32;
                if force != 0 {
                    append(s.key(force, 0, 0));
                }
                let mut converting = false;
                if c["action"] != "enter" {
                    let separate = c["action"] == "convert";
                    let started = s.key(
                        if separate { 28 } else { 32 },
                        if separate { 0 } else { 32 },
                        i32::from(c["action"] == "shift-space"),
                    );
                    converting = started.view.as_ref().is_some_and(|v| v.converting);
                    append(started);
                    if converting {
                        // Supply the same deterministic external-converter
                        // response as the original host, after the key branch.
                        let reading = s.reading();
                        let target = c["target"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("【{reading}】"));
                        if structured {
                            let expected = s.conversion_reading();
                            let mut clause = Clause::new(expected.clone(), vec![target]);
                            if expected != reading && expected.is_ascii() {
                                clause.english = true;
                                clause.kana = Some(reading);
                            }
                            assert!(s.begin_clauses(vec![clause]));
                        } else {
                            s.clauses.clear();
                            s.candidates = vec![target];
                            s.selected = 0;
                        }
                        if c["target"].is_string() {
                            append(s.select_composing(0));
                        }
                    }
                }
                append(s.key(13, 0, 0));
                let batches: Vec<_> = s.take_engine_learning().into_iter().map(|batch| serde_json::json!({"context":batch.context,"clauses":batch.clauses})).collect();
                assert_eq!(
                    serde_json::json!({"converting":converting,"committed":committed,"batches":batches}),
                    row["expected"],
                    "{} structured={structured}",
                    c["name"]
                );
            }
        }
    }
    #[test]
    fn original_controller_learning_runs_and_context_match() {
        let golden: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/data/upstream-engine-learning.json"
        ))
        .unwrap();
        let cases = golden["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 22);
        for row in cases {
            let case = &row["case"];
            let mut s = Session::default();
            s.settings
                .set("LearningEnabled", case["learning"].clone())
                .unwrap();
            s.no_learning = case["private"] == true;
            s.collect_engine_learning(true);
            s.before = case["before"].as_str().unwrap().into();
            let clauses = case["clauses"].as_array().unwrap();
            s.raw = clauses.iter().filter_map(|c| c["raw"].as_str()).collect();
            s.clauses = clauses
                .iter()
                .map(|c| {
                    s.translation_candidates.extend(
                        c["translations"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|t| t.as_str().unwrap().to_owned()),
                    );
                    let mut clause = Clause::new(
                        c["reading"].as_str().unwrap().into(),
                        vec![c["text"].as_str().unwrap().into()],
                    );
                    clause.english = c["english"] == true;
                    clause.raw = c["raw"].as_str().map(str::to_owned);
                    clause.translations = c["translations"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|t| t.as_str().unwrap().to_owned())
                        .collect();
                    clause
                })
                .collect();
            s.candidates = vec!["converting".into()];
            s.record_engine_learning();
            let actual: Vec<_> = s
                .take_engine_learning()
                .into_iter()
                .map(|batch| serde_json::json!({"context":batch.context,"clauses":batch.clauses}))
                .collect();
            assert_eq!(
                serde_json::json!(actual),
                row["expected"],
                "{}",
                case["name"]
            );
        }
    }
    #[test]
    fn engine_learning_groups_japanese_runs_and_keeps_readonly_fields_unlearned() {
        let mut s = Session::default();
        s.settings
            .set("LearningEnabled", serde_json::json!(true))
            .unwrap();
        s.collect_engine_learning(true);
        s.before = "前の文。 日本語 ".into();
        s.raw = "hashiapi".into();
        s.clauses = vec![
            Clause::new("はし".into(), vec!["橋".into()]),
            Clause::literal("api".into(), true),
            Clause::new("かみ".into(), vec!["紙".into()]),
            Clause::new("きょう".into(), vec!["月曜日".into()]),
            Clause::new("かける".into(), vec!["×".into()]),
        ];
        s.candidates = vec!["橋".into()];
        s.commit();
        let events = s.take_engine_learning();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].context, "日本語");
        assert_eq!(events[0].clauses[0].text, "橋");
        assert_eq!(events[1].clauses[0].text, "紙");
        assert!(s.take_engine_learning().is_empty());
    }
    #[test]
    fn context_and_exclusions_follow_original_controller() {
        assert_eq!(conversion_context("英語word"), "");
        assert_eq!(conversion_context("前。日本語"), "日本語");
        assert_eq!(
            conversion_context("一二三四五六七八九十日本語"),
            "四五六七八九十日本語"
        );
        assert!(!eligible(false, "abc", "語", "raw", &[]));
        assert!(!eligible(false, "ご", "raw", "raw", &[]));
        assert!(!eligible(false, "ご", "word", "raw", &["word".into()]));
        for block in 0..3 {
            let mut s = Session::default();
            s.settings
                .set("LearningEnabled", serde_json::json!(block != 0))
                .unwrap();
            s.private = block == 1;
            s.no_learning = block == 2;
            s.collect_engine_learning(true);
            s.raw = "hashi".into();
            s.candidates = vec!["橋".into()];
            s.commit();
            assert!(s.take_engine_learning().is_empty());
        }
    }
}
