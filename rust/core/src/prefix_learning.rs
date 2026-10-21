// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! UserModel's bounded prefix statistics. Distinct from composition language
//! choices: only the first 3..6 lowercase letters influence the ScoreEngine.
use crate::{detection::Contribution, input_session::Outcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Stats {
    pub japanese: u32,
    pub english: u32,
    pub last_used: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Model {
    pub version: u32,
    pub prefixes: BTreeMap<String, Stats>,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            version: 1,
            prefixes: BTreeMap::new(),
        }
    }
}
pub fn key(letters: &str) -> Option<&str> {
    (letters.len() >= 3 && letters.bytes().all(|b| b.is_ascii_lowercase()))
        .then(|| &letters[..letters.len().min(6)])
}
impl Stats {
    fn instant(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(&self.last_used)
            .map(|t| t.with_timezone(&chrono::Utc))
            .unwrap_or(chrono::DateTime::<chrono::Utc>::MIN_UTC)
    }
    pub fn used(&self) -> i64 {
        self.instant().timestamp()
    }
}
impl Model {
    pub fn parse(bytes: &[u8]) -> serde_json::Result<Self> {
        let mut value: serde_json::Value = serde_json::from_slice(bytes)?;
        if !value.is_object() {
            return Err(serde_json::Error::io(std::io::Error::other(
                "invalid prefix model",
            )));
        }
        if value["prefixes"].is_null() {
            value["prefixes"] = serde_json::json!({});
        }
        if let Some(entries) = value["prefixes"].as_object_mut() {
            entries.retain(|_, stats| !stats.is_null());
        }
        let mut model: Self = serde_json::from_value(value)?;
        model.prefixes.retain(|s, _| key(s) == Some(s.as_str()));
        Ok(model)
    }
    pub fn contribution(&self, letters: &str) -> Option<Contribution> {
        if !letters.is_ascii() {
            return None;
        }
        let (matched, stats) = (3..=letters.len().min(6)).rev().find_map(|len| {
            self.prefixes
                .get(&letters[..len])
                .map(|s| (&letters[..len], s))
        })?;
        let japanese = 2 * i64::from(stats.japanese) - 3 * i64::from(stats.english);
        let english = 2 * i64::from(stats.english) - i64::from(stats.japanese);
        let (japanese, english, language) = if japanese >= 4 {
            ((japanese / 2 + 2).min(4) as i32, 0, "日本語")
        } else if english >= 2 {
            (0, (english * 3).min(10) as i32, "英語")
        } else {
            return None;
        };
        Some(Contribution {
            source: "User".into(),
            japanese,
            english,
            reason: format!(
                "「{matched}」は{language}として使われてきた ({}/{})",
                stats.japanese, stats.english
            ),
        })
    }
    pub fn learn(
        &mut self,
        letters: &str,
        outcome: Outcome,
        decided_english: bool,
        time: chrono::DateTime<chrono::Utc>,
    ) -> bool {
        let Some(key) = key(letters) else {
            return false;
        };
        if !self.prefixes.contains_key(key)
            && outcome == Outcome::StayedAccepted
            && !decided_english
        {
            return false;
        }
        let stats = self.prefixes.entry(key.into()).or_default();
        match outcome {
            Outcome::JapaneseAccepted => stats.japanese = stats.japanese.saturating_add(1),
            Outcome::JapaneseRejected => {
                stats.english = stats.english.saturating_add(2);
                stats.japanese = stats.japanese.saturating_sub(1);
            }
            Outcome::StayedAccepted => stats.english = stats.english.saturating_add(1),
            Outcome::StayedRejected => {
                stats.japanese = stats.japanese.saturating_add(2);
                stats.english = stats.english.saturating_sub(1);
            }
        }
        stats.japanese = stats.japanese.min(1000);
        stats.english = stats.english.min(1000);
        stats.last_used = time.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        if self.prefixes.len() > 5000 {
            let mut oldest: Vec<_> = self
                .prefixes
                .iter()
                .map(|(k, v)| (k.clone(), v.instant()))
                .collect();
            oldest.sort_by_key(|entry| entry.1);
            for (k, _) in oldest.into_iter().take(self.prefixes.len() - 4500) {
                self.prefixes.remove(&k);
            }
        }
        true
    }
}
