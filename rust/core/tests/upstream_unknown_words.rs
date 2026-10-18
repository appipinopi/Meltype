// SPDX-License-Identifier: GPL-3.0-or-later
// Golden key traces captured from the unmodified upstream C# NativeAOT core.
use meltype_core::Session;
use serde_json::{json, Value};

#[test]
fn unknown_words_particles_small_kana_and_double_n_match_upstream() {
    let golden: Value =
        serde_json::from_str(include_str!("data/upstream-unknown-words.json")).unwrap();
    for case in golden["cases"].as_array().unwrap() {
        let mut session = Session::default();
        session.can_delete = true;
        for setting in ["LiveConversion", "PredictiveCandidates", "LearningEnabled"] {
            session.settings.set(setting, json!(false)).unwrap();
        }
        for (index, (key, expected)) in case["keys"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
            .enumerate()
        {
            let r = session.key(
                key[0].as_i64().unwrap() as i32,
                key[1].as_i64().unwrap() as i32,
                key[2].as_i64().unwrap() as i32,
            );
            let actual = json!({"consumed":r.consumed,"commits":r.commits,
                "text":r.view.as_ref().map(|v|v.text.as_str()),"converting":r.view.as_ref().is_some_and(|v|v.converting)});
            assert_eq!(&actual, expected, "{} at key {}", case["name"], index);
        }
    }
}
