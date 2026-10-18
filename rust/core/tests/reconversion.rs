// SPDX-License-Identifier: GPL-3.0-or-later
use meltype_core::Session;
use serde_json::{json, Value};

#[test]
fn romaji_plan_matches_original_conversion_requests() {
    let golden: Value =
        serde_json::from_str(include_str!("data/upstream-reconversion.json")).unwrap();
    let s = Session::default();
    for row in golden["cases"].as_array().unwrap() {
        let case = &row["case"];
        let text = case["text"].as_str().unwrap();
        if !text.is_ascii() {
            continue;
        }
        let mut converted = String::new();
        for part in s.romaji_reconversion_parts(text).unwrap() {
            if part.english {
                if !converted.is_empty() && !converted.ends_with(' ') {
                    converted.push(' ');
                }
                converted.push_str(&part.text);
                converted.push(' ');
            } else if case["engine"] == "bracket" {
                converted.push_str(&format!("【{}】", part.text));
            } else {
                converted.push_str(&part.text);
            }
        }
        assert_eq!(converted.trim_end(), row["converted"], "{}", case["name"]);
    }
}

#[test]
fn explicit_reconversion_works_in_direct_mode_and_preserves_original_choice() {
    let mut s = Session::default();
    s.direct = true;
    let start = s.begin_romaji_reconversion("kyouha meeting", "今日は meeting");
    assert!(start.consumed);
    assert!(start.commits.is_empty());
    assert_eq!(start.selection_expect.as_deref(), Some("kyouha meeting"));
    assert_eq!(s.reading(), "今日は meeting");
    assert_eq!(s.conversion_reading(), "今日は meeting");
    assert_eq!(s.key(0x28, 0, 0).view.unwrap().text, "kyouha meeting");
    assert_eq!(s.key(0x0d, 0, 0).commits[0].text, "kyouha meeting");
    assert!(s.direct);
    assert!(!s.key(0, 'a' as i32, 0).consumed);
}

#[test]
fn cancel_restores_original_and_reset_abandons_without_edit() {
    let mut s = Session::default();
    s.begin_romaji_reconversion("nihongo", "日本語");
    let count = s.state.phrases.len();
    assert_eq!(s.key(0x1b, 0, 0).commits[0].text, "nihongo");
    assert_eq!(s.state.phrases.len(), count);
    s.begin_romaji_reconversion("nihongo", "日本語");
    s.reset();
    assert!(s.key(0x0d, 0, 0).commits.is_empty());
    assert!(s.raw().is_empty());
}

#[test]
fn reconversion_rejects_private_disabled_busy_and_invalid_requests() {
    let mut s = Session::default();
    for text in ["", "   ", "123", "日本語", "a\nb", &"a".repeat(1025)] {
        assert!(s.romaji_reconversion_parts(text).is_none());
    }
    assert!(!s.begin_romaji_reconversion("nihongo", "nihongo").consumed);
    assert!(!s.begin_romaji_reconversion("nihongo", "").consumed);
    assert!(
        !s.begin_romaji_reconversion("nihongo", &"あ".repeat(8193))
            .consumed
    );
    s.private = true;
    assert!(!s.begin_romaji_reconversion("nihongo", "日本語").consumed);
    s.private = false;
    s.settings.set("Enabled", json!(false)).unwrap();
    assert!(!s.reconvert_selection("橋", Some("はし")).consumed);
    s.settings.set("Enabled", json!(true)).unwrap();
    s.key(0, 'a' as i32, 0);
    assert!(!s.begin_romaji_reconversion("nihongo", "日本語").consumed);
    assert_eq!(s.raw(), "a");
}

#[test]
fn kana_selection_reconversion_is_explicit_even_in_direct_mode() {
    let mut s = Session::default();
    s.direct = true;
    assert!(s.reconvert_selection("橋", Some("はし")).consumed);
    assert_eq!(s.key(0x1b, 0, 0).commits[0].text, "橋");
    assert!(s.direct);
    assert!(!s.reconvert_selection("橋", Some("not a reading")).consumed);
}
