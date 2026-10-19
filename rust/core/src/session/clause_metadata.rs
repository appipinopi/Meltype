// SPDX-License-Identifier: GPL-3.0-or-later
//! Provenance of a converter clause within the retained composition units.
use super::*;
impl Session {
    pub(super) fn clause_original_inputs(&self, clauses: &[Clause]) -> Vec<Option<String>> {
        if self.uses_kana() || self.reconversion_origin.is_some() {
            return vec![None; clauses.len()];
        }
        let reading: String = clauses.iter().map(|c| c.reading.as_str()).collect();
        let captured;
        let units = if let Some(units) = self.edited_units.as_ref().filter(|u| u.raw() == self.raw)
        {
            units
        } else {
            captured = crate::composition_units::Units::capture(&self.raw, &reading);
            &captured
        };
        let (settled, pending) = units.parts();
        let mut pieces: Vec<_> = settled
            .iter()
            .map(|u| (u.kana.clone(), u.spelling.clone()))
            .collect();
        if !pending.is_empty() {
            pieces.push((romaji::hiragana(pending, true), pending.into()));
        }
        let mut remaining = reading.as_str();
        let mut spans = Vec::new();
        let mut offset = 0;
        for (kana, raw) in pieces {
            let punctuated = self.punctuate(romaji::hiragana(&kana, true));
            let Some(output) = [kana.as_str(), raw.as_str(), punctuated.as_str()]
                .into_iter()
                .find(|s| !s.is_empty() && remaining.starts_with(s))
            else {
                return vec![None; clauses.len()];
            };
            let end = offset + output.encode_utf16().count();
            let consumed = output.len();
            spans.push((offset, end, raw));
            offset = end;
            remaining = &remaining[consumed..];
        }
        if !remaining.is_empty() {
            return vec![None; clauses.len()];
        }
        let mut start = 0;
        clauses
            .iter()
            .map(|clause| {
                let end = start + clause.reading.encode_utf16().count();
                let mut value = String::new();
                let mut partial = false;
                for (from, to, raw) in &spans {
                    if *to > start && *from < end {
                        partial |= *from < start || *to > end;
                        value.push_str(raw);
                    }
                }
                start = end;
                (!clause.english && !partial && !value.is_empty()).then_some(value)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_input_has_clause_local_raw_and_boundary_edits_clear_it() {
        let mut s = Session {
            raw: "hashikami".into(),
            ..Default::default()
        };
        s.settings
            .set("LearningEnabled", serde_json::json!(true))
            .unwrap();
        s.collect_engine_learning(true);
        assert!(s.begin_clauses(vec![
            Clause::new("はし".into(), vec!["hashi".into()]),
            Clause::new("かみ".into(), vec!["紙".into()])
        ]));
        assert_eq!(s.clauses[0].raw.as_deref(), Some("hashi"));
        assert_eq!(s.clauses[1].raw.as_deref(), Some("kami"));
        s.record_engine_learning();
        let learned = s.take_engine_learning();
        assert_eq!(learned.len(), 1);
        assert_eq!(learned[0].clauses[0].text, "紙");
        s.commit();
        assert!(!s.preferences.selected.contains_key("はし"));
        assert!(!s.preferences.selected.contains_key("はしかみ"));
        assert_eq!(
            s.preferences.selected.get("かみ").map(String::as_str),
            Some("紙")
        );
        s.raw = "hashikami".into();
        assert!(s.begin_clauses(vec![
            Clause::new("はし".into(), vec!["hashi".into()]),
            Clause::new("かみ".into(), vec!["紙".into()])
        ]));
        s.resize_clause(true);
        assert!(s.clauses.iter().all(|c| c.raw.is_none()));
    }
    #[test]
    fn translations_are_recorded_per_clause_and_not_as_engine_learning() {
        let mut s = Session {
            raw: "hashikami".into(),
            ..Default::default()
        };
        s.settings
            .set("LearningEnabled", serde_json::json!(true))
            .unwrap();
        s.settings
            .set("TranslationCandidates", serde_json::json!(true))
            .unwrap();
        s.collect_engine_learning(true);
        assert!(s.begin_clauses(vec![
            Clause::new("はし".into(), vec!["橋".into()]),
            Clause::new("かみ".into(), vec!["紙".into()])
        ]));
        let word = s.clauses[0]
            .translations
            .first()
            .expect("bundled translation for 橋")
            .clone();
        s.clauses[0].selected = s.clauses[0]
            .candidates
            .iter()
            .position(|t| t == &word)
            .unwrap();
        s.sync_clause();
        s.commit();
        assert!(s.state.translations["はし"][&word] > 0);
        assert_ne!(s.preferences.selected.get("はし"), Some(&word));
        let learned = s.take_engine_learning();
        assert_eq!(learned.len(), 1);
        assert_eq!(learned[0].clauses[0].text, "紙");
    }
    #[test]
    fn original_sound_unit_raw_ranges_match() {
        let golden: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/data/upstream-clause-raw.json"))
                .unwrap();
        assert_eq!(golden["cases"].as_array().unwrap().len(), 36);
        let mut checked = 0;
        for row in golden["cases"].as_array().unwrap() {
            let s = Session {
                raw: row["case"]["raw"].as_str().unwrap().into(),
                ..Default::default()
            };
            let segments = row["segments"].as_array().unwrap();
            for (index, segment) in segments.iter().enumerate() {
                let units: Vec<_> = segment["reading"]
                    .as_str()
                    .unwrap()
                    .encode_utf16()
                    .collect();
                for span in segment["spans"].as_array().unwrap() {
                    let start = span["start"].as_u64().unwrap() as usize;
                    let end = start + span["length"].as_u64().unwrap() as usize;
                    // Clause reading is a Rust string; surrogate-half cuts
                    // cannot be represented by this API. Keep those original
                    // rows in the fixture, and test all scalar boundaries.
                    let (Ok(prefix), Ok(target), Ok(suffix)) = (
                        String::from_utf16(&units[..start]),
                        String::from_utf16(&units[start..end]),
                        String::from_utf16(&units[end..]),
                    ) else {
                        continue;
                    };
                    let mut clauses = Vec::new();
                    let mut selected = 0;
                    for (i, part) in segments.iter().enumerate() {
                        if i == index {
                            if !prefix.is_empty() {
                                clauses.push(Clause::new(prefix.clone(), vec![]));
                            }
                            selected = clauses.len();
                            clauses.push(Clause::new(target.clone(), vec![]));
                            if !suffix.is_empty() {
                                clauses.push(Clause::new(suffix.clone(), vec![]));
                            }
                        } else {
                            let reading = part["reading"].as_str().unwrap().to_owned();
                            clauses.push(if part["english"] == true {
                                Clause::literal(reading, true)
                            } else {
                                Clause::new(reading, vec![])
                            });
                        }
                    }
                    let raw = s.clause_original_inputs(&clauses);
                    assert_eq!(
                        serde_json::json!(raw[selected]),
                        span["raw"],
                        "input {:?}, segment {index}, {start}..{end}",
                        row["case"]
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 334);
    }
}
