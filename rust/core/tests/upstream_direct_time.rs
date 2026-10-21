// SPDX-License-Identifier: GPL-3.0-or-later
//! Golden values come from the original C# session with its real ScoreEngine.
use meltype_core::Session;
use serde_json::{json, Value};

fn session() -> Session {
    let mut s = Session::default();
    s.can_delete = true;
    s.direct_detection = true;
    s.set_direct(true);
    for name in ["LearningEnabled", "LiveConversion", "PredictiveCandidates"] {
        s.settings.set(name, json!(false)).unwrap();
    }
    s
}

#[test]
fn original_timed_direct_input_matches_each_edit_mode_and_application_document() {
    let golden: Value =
        serde_json::from_str(include_str!("data/upstream-direct-time.json")).unwrap();
    let cases = golden["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 32);
    for case in cases {
        let mut s = session();
        let mut document = case["before"].as_str().unwrap_or("").to_owned();
        let steps = case["steps"].as_array().unwrap();
        let expected = case["expected"].as_array().unwrap();
        assert_eq!(steps.len(), expected.len());
        for (index, (step, expected)) in steps.iter().zip(expected).enumerate() {
            if let Some(before) = step["before"].as_str() {
                document = before.into();
            }
            if step["moved"] == true {
                s.caret_moved(0, false);
            }
            if let Some(direct) = step["direct"].as_bool() {
                s.set_direct(direct);
            }
            s.set_context(Some(&document), None);
            let ch = step["ch"].as_i64().unwrap() as i32;
            let result = s.key_at(
                step["vk"].as_i64().unwrap() as i32,
                ch,
                step["mods"].as_i64().unwrap() as i32,
                step["time"].as_u64().unwrap(),
            );
            for edit in &result.commits {
                if edit.delete_before > 0 {
                    let expected_text = edit.expect.as_deref().expect("deletion must be guarded");
                    assert!(document.ends_with(expected_text));
                    assert_eq!(edit.delete_before, expected_text.encode_utf16().count());
                    document.truncate(document.len() - expected_text.len());
                }
                document.push_str(&edit.text);
            }
            if !result.consumed && ch >= 32 {
                document.push(char::from_u32(ch as u32).unwrap());
            }
            assert_eq!(
                json!({"consumed":result.consumed,"commits":result.commits,
                "text":result.view.as_ref().map(|v|v.text.as_str()),"direct":s.direct,
                "english":s.direct_english_word,"document":document}),
                *expected,
                "{} at step {index}",
                case["name"]
            );
        }
    }
}

#[test]
fn timeout_never_deletes_unverified_text_and_private_input_drops_pending_detection() {
    for private in [false, true] {
        let mut s = session();
        for (i, c) in "kon".char_indices() {
            s.set_context(Some(&"kon"[..i]), None);
            assert!(
                !s.key_at(c.to_ascii_uppercase() as i32, c as i32, 0, i as u64 * 20)
                    .consumed
            );
        }
        s.private = private;
        s.set_context(Some("unrelated 😀"), None);
        let result = s.key_at(78, 'n' as i32, 0, 740);
        assert!(result.commits.is_empty());
        assert!(!result.consumed);
    }
}
