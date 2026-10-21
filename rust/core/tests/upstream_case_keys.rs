// SPDX-License-Identifier: GPL-3.0-or-later
use meltype_core::Session;
use serde_json::{json, Value};
#[test]
fn original_f9_f10_case_cycles_match() {
    let fixture: Value =
        serde_json::from_str(include_str!("data/upstream-case-keys.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 96);
    for row in cases {
        let case = &row["case"];
        let mut s = Session::default();
        for name in [
            "LiveConversion",
            "PredictiveCandidates",
            "LearningEnabled",
            "SigilWordsDirect",
        ] {
            s.settings.set(name, json!(false)).unwrap();
        }
        s.settings
            .set("DetectionLevel", case["level"].clone())
            .unwrap();
        for ch in case["raw"].as_str().unwrap().chars() {
            s.key(
                ch.to_ascii_uppercase() as i32,
                ch as i32,
                i32::from(ch.is_ascii_uppercase()),
            );
        }
        for (key, expected) in case["keys"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["expected"].as_array().unwrap())
        {
            let result = s.key(key.as_i64().unwrap() as i32, 0, 0);
            assert!(result.commits.is_empty());
            assert_eq!(
                json!({"consumed":result.consumed,"text":result.view.as_ref().map(|v|v.text.as_str()),"converting":result.view.as_ref().is_some_and(|v|v.converting)}),
                *expected,
                "{} key {key}",
                case["name"]
            );
        }
    }
}
