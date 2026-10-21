// SPDX-License-Identifier: GPL-3.0-or-later
use meltype_core::{
    settings::Settings,
    storage::{Change, Store},
    Session,
};
use serde_json::json;
fn type_text(s: &mut Session, text: &str) {
    for c in text.chars() {
        s.key(0, c as i32, 0);
    }
}
#[test]
fn contextual_choices_survive_reload_without_overwriting_global_or_other_context() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::new(directory.path());
    let mut settings = Settings::default();
    settings.set("ContextualLearning", json!(true)).unwrap();
    settings
        .set("AlphanumericKeyStartsAsTyped", json!(true))
        .unwrap();
    std::fs::write(
        directory.path().join("config.json"),
        serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    store
        .record(vec![Change::Language {
            raw: "hashi".into(),
            english: false,
            explicit: true,
        }])
        .unwrap();
    let mut s = Session::open(store.clone()).unwrap();
    for _ in 0..2 {
        s.set_context(Some("hello "), Some(""));
        type_text(&mut s, "hashi");
        s.key(121, 0, 0);
        s.commit();
    }
    store
        .record(vec![
            Change::Language {
                raw: "hashi".into(),
                english: true,
                explicit: true,
            },
            Change::Language {
                raw: "hashi".into(),
                english: true,
                explicit: true,
            },
        ])
        .unwrap();
    s = Session::open(store.clone()).unwrap();
    s.set_context(Some("日本語"), Some(""));
    type_text(&mut s, "hashi");
    s.key(117, 0, 0);
    s.commit();
    let state = store.load().unwrap();
    assert!(state.language_counts["hashi"].english);
    assert!(state.language_counts["hashi@e"].english);
    assert!(!state.language_counts["hashi@j"].english);
    let mut reopened = Session::open(store).unwrap();
    reopened.set_context(Some("hello "), Some(""));
    type_text(&mut reopened, "hashi");
    assert_eq!(reopened.display(false), "hashi");
    reopened.reset();
    reopened.set_context(Some("日本語"), Some(""));
    type_text(&mut reopened, "hashi");
    assert_eq!(reopened.display(false), "はし");
}
#[test]
fn delete_prediction_forgets_all_phrase_readings_but_preserves_user_dictionary() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::new(directory.path());
    let word = "今日はいい天気";
    store
        .record(vec![
            Change::Phrase {
                reading: "きょうは".into(),
                text: word.into(),
            },
            Change::Phrase {
                reading: "きょうはいい".into(),
                text: word.into(),
            },
            Change::Phrase {
                reading: "きょうの".into(),
                text: "今日の予定".into(),
            },
        ])
        .unwrap();
    store
        .update(|state| {
            state
                .preferences
                .user_dictionary
                .insert("きょうは".into(), vec![word.into()]);
            Ok(())
        })
        .unwrap();
    let mut s = Session::open(store.clone()).unwrap();
    s.settings.set("LearningEnabled", json!(false)).unwrap();
    type_text(&mut s, "kyou");
    s.key(9, 0, 0);
    assert!(s.key(46, 0, 0).consumed);
    let state = store.load().unwrap();
    assert!(state.phrases.iter().all(|p| p.text != word));
    assert!(state.phrases.iter().any(|p| p.text == "今日の予定"));
    assert_eq!(state.preferences.user_dictionary["きょうは"], [word]);
    assert!(s.predictions().contains(&word.to_owned()));
}

#[test]
fn intervals_are_opt_in_bounded_and_contain_no_keys_or_typed_content() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::new(directory.path());
    let mut s = Session::open(store).unwrap();
    s.key_at(65, 65, 0, 10);
    assert!(s.key_intervals().is_empty());
    s.settings.set("LogKeyIntervals", json!(true)).unwrap();
    s.key_at(65, 65, 0, 100);
    s.key_at(16, 0, 1, 150);
    s.key_at(66, 66, 0, 200);
    assert_eq!(
        s.key_intervals().back().unwrap(),
        &json!({"intervalMs":100,"composing":true})
    );
    assert!(!directory.path().join("key-intervals.jsonl").exists());
    s.settings.set("FileLog", json!(true)).unwrap();
    s.key_at(13, 0, 0, 300);
    let text = std::fs::read_to_string(directory.path().join("key-intervals.jsonl")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(text.trim()).unwrap(),
        json!({"intervalMs":100,"composing":true})
    );
    for i in 0..140 {
        s.key_at(0, 0, 0, 400 + i);
    }
    assert_eq!(s.key_intervals().len(), 128);
    s.private = true;
    s.key_at(67, 67, 0, 1000);
    assert_eq!(s.key_intervals().len(), 128);
}
#[test]
fn clipboard_is_conversion_scoped_and_never_enters_durable_learning() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::new(directory.path());
    let mut s = Session::open(store.clone()).unwrap();
    s.settings.set("ClipboardCandidates", json!(true)).unwrap();
    type_text(&mut s, "pe-suto");
    assert!(s.wants_clipboard_candidate());
    s.set_clipboard_candidate(Some(" 機密の候補 "));
    s.key(32, 32, 0);
    let view = s.result(false).view.unwrap();
    let index = view
        .candidates
        .iter()
        .position(|v| v == "機密の候補")
        .unwrap();
    s.select(index);
    let state = store.load().unwrap();
    assert!(!serde_json::to_string(&state).unwrap().contains("機密"));
    assert!(!s.wants_clipboard_candidate());
    type_text(&mut s, "pe-suto");
    s.set_clipboard_candidate(Some("bad\nline"));
    s.key(32, 32, 0);
    assert!(s
        .result(false)
        .view
        .unwrap()
        .candidates
        .iter()
        .all(|v| !v.contains("bad")));
}

#[test]
fn new_settings_keep_profile_values_types_and_shared_enable_state() {
    let mut settings = Settings::default();
    settings.normalize();
    let original = settings.text("ActiveProfile").to_owned();
    settings.set("CalculatorCandidates", json!(true)).unwrap();
    settings.set("EnglishNamePredictions", json!(true)).unwrap();
    settings
        .set("GitHubRepository", json!("owner/first"))
        .unwrap();
    settings.add_profile("work").unwrap();
    settings.set("CalculatorCandidates", json!(false)).unwrap();
    settings
        .set("GitHubRepository", json!("owner/second"))
        .unwrap();
    settings.set("Enabled", json!(false)).unwrap();
    assert!(settings
        .set("EnglishNamePredictions", json!("invalid bool"))
        .is_err());
    assert!(settings.set("PauseHotkey", json!(true)).is_err());
    settings.switch_profile(&original).unwrap();
    assert!(settings.boolean("CalculatorCandidates"));
    assert!(settings.boolean("EnglishNamePredictions"));
    assert_eq!(settings.text("GitHubRepository"), "owner/first");
    assert!(!settings.boolean("Enabled"));
    let serialized = serde_json::to_string(&settings).unwrap();
    let mut reopened = Settings::parse(&serialized).unwrap();
    reopened.switch_profile("work").unwrap();
    assert!(!reopened.boolean("CalculatorCandidates"));
    assert_eq!(reopened.text("GitHubRepository"), "owner/second");
    assert!(!reopened.boolean("Enabled"));
}
