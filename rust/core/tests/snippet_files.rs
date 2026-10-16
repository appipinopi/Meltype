// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
use meltype_core::{control, storage::Store};
use serde_json::json;

#[test]
fn upstream_snippet_files_merge_replace_normalize_and_round_trip_without_losing_other_data() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path().join("data"));
    control::execute(
        &store,
        &json!({"op":"snippet_set","name":"sig","text":"古い"}),
    )
    .unwrap();
    control::execute(
        &store,
        &json!({"op":"snippet_set","name":"keep","text":"残す"}),
    )
    .unwrap();
    control::execute(
        &store,
        &json!({"op":"dictionary_set","reading":"じしょ","word":"辞書"}),
    )
    .unwrap();
    let path = dir.path().join("元版の定型文.json");
    let source = json!({"sig":"--\r\n雪代\r\u{2028}末尾", "pr-template":"a\u{0085}b\u{000c}c\u{2029}d", "bad name":"x", "a;b":"x", "empty":"", "nil":null}).to_string();
    let utf16: Vec<_> = std::iter::once(0xfeff_u16)
        .chain(source.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    std::fs::write(&path, utf16).unwrap();
    let result = control::execute(&store, &json!({"op":"snippets_import","path":path})).unwrap();
    assert_eq!(result["imported"], 2);
    let state = store.load().unwrap();
    assert_eq!(state.snippets.len(), 3);
    assert_eq!(state.snippets["sig"], "--\n雪代\n\n末尾");
    assert_eq!(state.snippets["pr-template"], "a\nb\nc\nd");
    assert_eq!(state.preferences.user_dictionary["じしょ"], ["辞書"]);
    let exported = dir.path().join("export.json");
    control::execute(&store, &json!({"op":"snippets_export","path":exported})).unwrap();
    let other = Store::new(dir.path().join("other"));
    control::execute(&other, &json!({"op":"snippets_import","path":exported})).unwrap();
    assert_eq!(other.load().unwrap().snippets, state.snippets);
    let mut session = meltype_core::Session::open(other).unwrap();
    for c in ";sig".chars() {
        session.key(0, c as i32, 0);
    }
    let commit = session.key(32, 32, 0);
    assert!(commit.commits.iter().any(|c| c.text == "--\n雪代\n\n末尾"));
}

#[test]
fn malformed_snippet_files_are_rejected_before_any_state_write() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path().join("data"));
    control::execute(
        &store,
        &json!({"op":"snippet_set","name":"sig","text":"保存した文"}),
    )
    .unwrap();
    let before = std::fs::read(store.directory.join("state.json")).unwrap();
    let path = dir.path().join("bad.json");
    for source in [r#"{"sig":"置き換え", "bad":42}"#, r#"{"sig":"途中""#, "[]"] {
        std::fs::write(&path, source).unwrap();
        assert!(control::execute(&store, &json!({"op":"snippets_import","path":path})).is_err());
        assert_eq!(
            std::fs::read(store.directory.join("state.json")).unwrap(),
            before
        );
    }
    std::fs::write(&path, b"\xef\xbb\xbfnull").unwrap();
    assert_eq!(
        control::execute(&store, &json!({"op":"snippets_import","path":path})).unwrap()["imported"],
        0
    );
    assert_eq!(
        std::fs::read(store.directory.join("state.json")).unwrap(),
        before
    );
}
