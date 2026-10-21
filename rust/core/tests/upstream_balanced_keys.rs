// SPDX-License-Identifier: GPL-3.0-or-later
//! Buffered Balanced key traces captured from the original C# NativeAOT host.
//! This does not assert candidate/UI, direct-input or all-setting equivalence.
use meltype_core::Session;
use serde_json::{json, Value};

#[test]
fn original_balanced_words_edits_and_capitalization_match_at_every_key() {
    let golden: Value =
        serde_json::from_str(include_str!("data/upstream-balanced-keys.json")).unwrap();
    let cases = golden["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 75);
    for case in cases {
        let mut session = Session::default();
        session.can_delete = true;
        for setting in ["LiveConversion", "PredictiveCandidates", "LearningEnabled"] {
            session.settings.set(setting, json!(false)).unwrap();
        }
        let keys = case["keys"].as_array().unwrap();
        let expected = case["expected"].as_array().unwrap();
        assert_eq!(keys.len(), expected.len());
        for (index, (key, expected)) in keys.iter().zip(expected).enumerate() {
            let result = session.key(
                key[0].as_i64().unwrap() as i32,
                key[1].as_i64().unwrap() as i32,
                key[2].as_i64().unwrap() as i32,
            );
            assert_eq!(
                json!({"consumed":result.consumed,"commits":result.commits,
                    "text":result.view.as_ref().map(|v|v.text.as_str()),
                    "converting":result.view.as_ref().is_some_and(|v|v.converting)}),
                *expected,
                "{} at key {index}",
                case["name"]
            );
        }
    }
}
