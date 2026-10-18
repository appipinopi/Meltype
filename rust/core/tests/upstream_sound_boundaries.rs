// SPDX-License-Identifier: GPL-3.0-or-later
//! Selected regression traces from the original C# host with bundled spelling.
//! The broader 61-case diagnostic still records incomplete intermediate parity.
use meltype_core::Session;
use serde_json::{json, Value};

#[test]
fn english_spans_respect_sound_units_and_partial_japanese_after_an_edit() {
    let golden: Value =
        serde_json::from_str(include_str!("data/upstream-sound-boundaries.json")).unwrap();
    for case in golden["cases"].as_array().unwrap() {
        let mut session = Session::default();
        session.can_delete = true;
        for setting in ["LiveConversion", "PredictiveCandidates", "LearningEnabled"] {
            session.settings.set(setting, json!(false)).unwrap();
        }
        let keys = case["keys"].as_array().unwrap();
        let expected = case["expected"].as_array().unwrap();
        assert_eq!(keys.len(), expected.len());
        for (index, (key, expected)) in keys.iter().zip(expected).enumerate() {
            let r = session.key(
                key[0].as_i64().unwrap() as i32,
                key[1].as_i64().unwrap() as i32,
                key[2].as_i64().unwrap() as i32,
            );
            assert_eq!(
                json!({"consumed":r.consumed,"commits":r.commits,
                    "text":r.view.as_ref().map(|v|v.text.as_str()),
                    "converting":r.view.as_ref().is_some_and(|v|v.converting)}),
                *expected,
                "{} at key {index}",
                case["name"]
            );
        }
    }
}

#[test]
fn english_suffix_y_protection_requires_a_whole_word() {
    let mut s = Session::default();
    for setting in ["LiveConversion", "PredictiveCandidates", "LearningEnabled"] {
        s.settings.set(setting, json!(false)).unwrap();
    }
    for c in "thankyouforyourhelp".chars() {
        s.key(c.to_ascii_uppercase() as i32, c as i32, 0);
    }
    // Original final output. Intermediate displays remain in the 61-case audit.
    assert_eq!(s.commit().commits[0].text, "thankyouふぉりょうrhelp");
}
