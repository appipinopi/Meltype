// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
use meltype_core::{control, settings::Settings, storage::Store};
use serde_json::json;
fn settings() -> Settings {
    let mut s = Settings::default();
    s.normalize();
    s
}
#[test]
fn unchanged_window_preserves_current_settings_and_unknown_values() {
    let opened = settings();
    let mut current = opened.clone();
    current.set("Enabled", json!(false)).unwrap();
    current.set("FutureSetting", json!({"keep":true})).unwrap();
    assert_eq!(
        Settings::merge_edits(&opened, &opened, &current).0,
        current.0
    );
}
#[test]
fn window_edits_merge_into_original_profile_after_external_switch() {
    let mut opened = settings();
    opened.add_profile("仕事").unwrap();
    let mut edited = opened.clone();
    edited.set("LiveConversion", json!(false)).unwrap();
    let mut current = opened.clone();
    current.set("Enabled", json!(false)).unwrap();
    current.switch_profile("仕事").unwrap();
    current.set("JapaneseThreshold", json!(9)).unwrap();
    let mut merged = Settings::merge_edits(&opened, &edited, &current);
    assert!(!merged.boolean("Enabled"));
    assert_eq!(merged.text("ActiveProfile"), "仕事");
    assert_eq!(merged.integer("JapaneseThreshold"), 9);
    assert!(merged.boolean("LiveConversion"));
    merged.switch_profile("標準").unwrap();
    assert!(!merged.boolean("LiveConversion"));
    assert_eq!(merged.integer("JapaneseThreshold"), 4);
}
#[test]
fn edited_profile_switch_and_addition_preserve_external_edits_in_each_profile() {
    let mut opened = settings();
    opened.add_profile("仕事").unwrap();
    let mut edited = opened.clone();
    edited.switch_profile("仕事").unwrap();
    edited.set("LiveConversion", json!(false)).unwrap();
    edited.add_profile("趣味").unwrap();
    let mut current = opened.clone();
    current.set("PredictiveCandidates", json!(false)).unwrap();
    current.switch_profile("仕事").unwrap();
    current.set("JapaneseThreshold", json!(8)).unwrap();
    let mut merged = Settings::merge_edits(&opened, &edited, &current);
    assert_eq!(merged.text("ActiveProfile"), "仕事");
    assert!(!merged.boolean("LiveConversion"));
    assert_eq!(merged.integer("JapaneseThreshold"), 8);
    merged.switch_profile("標準").unwrap();
    assert!(!merged.boolean("PredictiveCandidates"));
    merged.switch_profile("趣味").unwrap();
    assert!(!merged.boolean("LiveConversion"));
}
#[test]
fn merge_management_api_is_atomic_and_preserves_other_process_changes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    let opened = store.settings().unwrap();
    let mut edited = opened.clone();
    edited.set("JapaneseThreshold", json!(7)).unwrap();
    store
        .change_settings(|s| s.set("Enabled", json!(false)))
        .unwrap();
    control::execute(
        &store,
        &json!({"op":"settings_merge","opened":opened,"edited":edited}),
    )
    .unwrap();
    let actual = store.settings().unwrap();
    assert_eq!(actual.integer("JapaneseThreshold"), 7);
    assert!(!actual.boolean("Enabled"));
    let bytes = std::fs::read(dir.path().join("config.json")).unwrap();
    assert!(control::execute(
        &store,
        &json!({"op":"settings_merge","opened":actual,"edited":"invalid"})
    )
    .is_err());
    assert_eq!(
        std::fs::read(dir.path().join("config.json")).unwrap(),
        bytes
    );
}

#[test]
fn upstream_profile_files_round_trip_and_cannot_overwrite_shared_settings() {
    let mut source = settings();
    source.set("LiveConversion", json!(false)).unwrap();
    source.set("JapaneseThreshold", json!(8)).unwrap();
    source.set("Enabled", json!(false)).unwrap();
    let file = source.export_profile();
    assert_eq!(file["format"], "meltype-profile");
    assert!(file["values"].get("Enabled").is_none());
    let mut target = settings();
    let name = target.import_profile(&file.to_string()).unwrap();
    assert_eq!(name, "標準 (2)");
    assert_eq!(target.text("ActiveProfile"), name);
    assert!(!target.boolean("LiveConversion"));
    assert!(target.boolean("Enabled"));
    assert_eq!(target.integer("JapaneseThreshold"), 8);
    let name = target.import_profile(r#"{"format":"meltype-profile","name":"仕事","values":{"Enabled":false,"JapaneseThreshold":999,"Unknown":1}}"#).unwrap();
    assert_eq!(name, "仕事");
    assert!(target.boolean("Enabled"));
    assert_eq!(target.integer("JapaneseThreshold"), 20);
    assert!(!target.0.contains_key("Unknown"));
    let before = target.0.clone();
    for file in [
        r#"{"format":"something-else","values":{}}"#,
        r#"{"format":"meltype-profile","values":{"LiveConversion":"yes"}}"#,
        r#"{"format":"meltype-profile","values":[]}"#,
    ] {
        assert!(target.import_profile(file).is_err());
        assert_eq!(target.0, before);
    }
}
#[test]
fn rename_and_remove_current_profile_use_upstream_rules_and_merge_on_save() {
    let opened = settings();
    let mut edited = opened.clone();
    edited.rename_profile("標準", "仕事").unwrap();
    assert_eq!(edited.text("ActiveProfile"), "仕事");
    assert!(edited.rename_profile("仕事", " ").is_err());
    assert!(edited.remove_profile("仕事").is_err());
    edited.add_profile("趣味").unwrap();
    assert!(edited.rename_profile("仕事", "趣味").is_err());
    let mut merged = Settings::merge_edits(&opened, &edited, &opened);
    assert_eq!(merged.text("ActiveProfile"), "仕事");
    merged.remove_profile("仕事").unwrap();
    assert_eq!(merged.text("ActiveProfile"), "趣味");
    assert!(merged.remove_profile("unknown").is_err());
}
