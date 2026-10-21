// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use meltype_core::{control, storage::Store};
use rusqlite::Connection;
use serde_json::{json, Value};
fn add(store: &Store, word: &str) {
    control::execute(
        store,
        &json!({"op":"dictionary_set","reading":"はし","word":word}),
    )
    .unwrap();
}
#[test]
fn legacy_dictionary_migrates_preserving_priority_and_sqlite_is_authoritative() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    std::fs::write(dir.path().join("userdict.txt"), "はし\t橋\nはし\t箸\n").unwrap();
    let original = store.load().unwrap().preferences.user_dictionary;
    assert!(!dir.path().join("userdict.sqlite3").exists());
    store.update(|_| Ok(())).unwrap();
    assert_eq!(
        Store::new(dir.path())
            .load()
            .unwrap()
            .preferences
            .user_dictionary,
        original
    );
    let db = Connection::open(dir.path().join("userdict.sqlite3")).unwrap();
    let integrity: String = db
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    db.execute(
        "UPDATE dictionary_entries SET word='端' WHERE reading='はし' AND position=0",
        [],
    )
    .unwrap();
    assert_eq!(
        store.load().unwrap().preferences.user_dictionary["はし"][0],
        "端"
    );
    let mirror: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("state.json")).unwrap()).unwrap();
    assert_eq!(mirror["preferences"]["user_dictionary"], json!({}));
    let before = std::fs::read(dir.path().join("userdict.sqlite3")).unwrap();
    store
        .update(|s| {
            s.snippets.insert("keep".into(), "text".into());
            Ok(())
        })
        .unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("userdict.sqlite3")).unwrap(),
        before,
        "unrelated learning must not rewrite dictionary rows"
    );
}
#[test]
fn empty_dictionary_does_not_resurrect_legacy_data_and_logical_backup_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    add(&store, "橋");
    add(&store, "箸");
    let want = store.load().unwrap().preferences.user_dictionary;
    let backup = store.backup().unwrap();
    let document: Value = serde_json::from_slice(&backup).unwrap();
    assert!(document["files"].get("userdict.sqlite3").is_none());
    store
        .update(|s| {
            s.preferences.user_dictionary.clear();
            Ok(())
        })
        .unwrap();
    std::fs::write(
        dir.path().join("userdict.txt"),
        "はし\t復活してはいけない\n",
    )
    .unwrap();
    assert!(Store::new(dir.path())
        .load()
        .unwrap()
        .preferences
        .user_dictionary
        .is_empty());
    store.restore(&backup).unwrap();
    assert_eq!(store.load().unwrap().preferences.user_dictionary, want);
    let other = tempfile::tempdir().unwrap();
    let other = Store::new(other.path());
    other.restore(&backup).unwrap();
    assert_eq!(other.load().unwrap().preferences.user_dictionary, want);
}
#[test]
fn corrupt_and_future_databases_fail_closed_without_overwriting_json_or_database() {
    for future in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        add(&store, "橋");
        let path = dir.path().join("userdict.sqlite3");
        if future {
            let db = Connection::open(&path).unwrap();
            db.execute_batch("PRAGMA user_version=999").unwrap();
        } else {
            std::fs::write(&path, b"broken database").unwrap();
        }
        let db = std::fs::read(&path).unwrap();
        let json = std::fs::read(dir.path().join("state.json")).unwrap();
        assert!(store.load().is_err());
        assert!(store
            .update(|s| {
                s.preferences.user_dictionary.clear();
                Ok(())
            })
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), db);
        assert_eq!(std::fs::read(dir.path().join("state.json")).unwrap(), json);
    }
}
#[test]
fn failed_restore_preflight_keeps_sqlite_and_json_unchanged() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    add(&store, "橋");
    let before = store.load().unwrap().preferences.user_dictionary;
    let bytes = std::fs::read(dir.path().join("userdict.sqlite3")).unwrap();
    std::fs::create_dir(dir.path().join("snippets.json")).unwrap();
    let backup = json!({"format":"meltype-backup","version":1,"files":{"userdict.txt":STANDARD.encode("はし\t別の候補\n"),"snippets.json":STANDARD.encode("{}")}});
    assert!(store
        .restore(&serde_json::to_vec(&backup).unwrap())
        .is_err());
    assert_eq!(store.load().unwrap().preferences.user_dictionary, before);
    assert_eq!(
        std::fs::read(dir.path().join("userdict.sqlite3")).unwrap(),
        bytes
    );
}
#[test]
fn concurrent_edits_keep_all_words_and_empty_reading_groups() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    let threads: Vec<_> = (0..8)
        .map(|n| {
            let store = store.clone();
            std::thread::spawn(move || add(&store, &format!("候補{n}")))
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(
        store.load().unwrap().preferences.user_dictionary["はし"].len(),
        8
    );
    store
        .update(|s| {
            s.preferences.user_dictionary.insert("から".into(), vec![]);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        store.load().unwrap().preferences.user_dictionary["から"],
        Vec::<String>::new()
    );
}
