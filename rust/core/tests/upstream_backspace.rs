// SPDX-License-Identifier: GPL-3.0-or-later
// Expected states captured from the unmodified upstream C# NativeAOT session.
use meltype_core::Session;
use serde_json::{json, Value};

#[test]
fn upstream_backspace_units_and_pending_letters() {
    let golden: Value = serde_json::from_str(include_str!("data/upstream-backspace.json")).unwrap();
    for case in golden["cases"].as_array().unwrap() {
        let typed = case["typed"].as_str().unwrap();
        let mut session = Session::default();
        session.can_delete = true;
        for setting in ["LiveConversion", "LearningEnabled", "PredictiveCandidates"] {
            session.settings.set(setting, json!(false)).unwrap();
        }
        for c in typed.chars() {
            session.key(
                if c.is_ascii_alphabetic() {
                    c.to_ascii_uppercase() as i32
                } else {
                    7
                },
                c as i32,
                i32::from(c.is_ascii_uppercase()),
            );
        }
        for (index, expected) in case["backspaces"].as_array().unwrap().iter().enumerate() {
            let result = session.key(8, 0, 0);
            assert_eq!(
                json!({"consumed":result.consumed,"text":result.view.map(|v|v.text)}),
                *expected,
                "{typed}: Backspace {}",
                index + 1
            );
        }
    }
}

#[test]
fn retained_units_survive_function_keys_and_new_input() {
    let mut session = Session::default();
    for c in "gakkou".chars() {
        session.key(0, c as i32, 0);
    }
    session.key(8, 0, 0);
    session.key(8, 0, 0);
    assert_eq!(session.reading(), "がっ");
    assert_eq!(session.key(0x76, 0, 0).view.unwrap().text, "ガッ");
    assert_eq!(session.commit().commits[0].text, "ガッ");
    for c in "gakkou".chars() {
        session.key(0, c as i32, 0);
    }
    session.key(8, 0, 0);
    session.key(8, 0, 0);
    for c in "kou".chars() {
        session.key(0, c as i32, 0);
    }
    assert_eq!(session.reading(), "がっこう");
    assert_eq!(session.commit().commits[0].text, "がっこう");
    for c in "gakkou".chars() {
        session.key(0, c as i32, 0);
    }
    session.key(8, 0, 0);
    session.key(8, 0, 0);
    for c in "ta".chars() {
        session.key(0, c as i32, 0);
    }
    assert_eq!(session.commit().commits[0].text, "gakta");
    for c in "google".chars() {
        session.key(0, c as i32, 0);
    }
    session.key(8, 0, 0);
    session.key(8, 0, 0);
    for c in "ya".chars() {
        session.key(0, c as i32, 0);
    }
    assert_eq!(session.commit().commits[0].text, "ごおぎゃ");
}
