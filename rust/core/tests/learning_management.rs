// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use meltype_core::{
    control,
    storage::{Change, Store},
};
use serde_json::json;

#[test]
fn forgetting_individual_learning_keeps_other_readings_dictionary_and_snippets() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    store
        .record(vec![
            Change::Language {
                raw: "sushi".into(),
                english: true,
                explicit: true,
            },
            Change::Conversion {
                reading: "はし".into(),
                text: "橋".into(),
                translation: false,
            },
            Change::Conversion {
                reading: "はし".into(),
                text: "bridge".into(),
                translation: true,
            },
            Change::Phrase {
                reading: "はしをわたる".into(),
                text: "橋を渡る".into(),
            },
            Change::Phrase {
                reading: "はしをわたる".into(),
                text: "端を渡る".into(),
            },
            Change::Phrase {
                reading: "ほかのぶん".into(),
                text: "他の文".into(),
            },
        ])
        .unwrap();
    control::execute(
        &store,
        &json!({"op":"dictionary_set","reading":"はし","word":"箸"}),
    )
    .unwrap();
    control::execute(
        &store,
        &json!({"op":"snippet_set","name":"sig","text":"署名"}),
    )
    .unwrap();
    for key in ["language:sushi", "conversion:はし", "phrase:はしをわたる"] {
        control::execute(&store, &json!({"op":"learning_remove","key":key})).unwrap();
    }
    let state = store.load().unwrap();
    assert!(state.language_counts.is_empty());
    assert!(state.preferences.english.is_empty());
    assert!(state.preferences.selected.is_empty());
    assert!(state.translations.is_empty());
    assert_eq!(state.phrases.len(), 1);
    assert_eq!(state.phrases[0].text, "他の文");
    assert_eq!(state.preferences.user_dictionary["はし"], ["箸"]);
    assert_eq!(state.snippets["sig"], "署名");
}
