// SPDX-License-Identifier: GPL-3.0-or-later
use meltype_core::{
    control,
    input_session::Outcome,
    prefix_learning::{key, Model},
    storage::Store,
};
use serde_json::json;

#[test]
fn persisted_false_positive_feedback_reaches_direct_detection_and_respects_learning_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    control::execute(
        &store,
        &json!({"op":"prefix_learn","letters":"kyo","outcome":"JapaneseRejected"}),
    )
    .unwrap();
    for enabled in [true, false] {
        let mut session = meltype_core::Session::open(store.clone()).unwrap();
        session
            .settings
            .set("LearningEnabled", json!(enabled))
            .unwrap();
        for name in ["LiveConversion", "PredictiveCandidates"] {
            session.settings.set(name, json!(false)).unwrap();
        }
        session.can_delete = true;
        session.direct_detection = true;
        session.set_direct(true);
        let mut document = String::new();
        for (i, c) in "kyou".chars().enumerate() {
            session.set_context(Some(&document), None);
            let result = session.key_at(c.to_ascii_uppercase() as i32, c as i32, 0, i as u64 * 10);
            for edit in result.commits {
                if edit.delete_before > 0 {
                    let expected = edit.expect.unwrap();
                    assert!(document.ends_with(&expected));
                    document.truncate(document.len() - expected.len());
                }
                document.push_str(&edit.text);
            }
            if !result.consumed {
                document.push(c);
            }
        }
        for edit in session.commit().commits {
            document.push_str(&edit.text);
        }
        assert_eq!(document, if enabled { "kyou" } else { "きょう" });
    }
}

#[test]
fn imports_original_model_once_and_clear_does_not_resurrect_legacy_data() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    std::fs::write(
        dir.path().join("model.json"),
        serde_json::to_vec(&json!({"version":1,"prefixes":{
            "konn":{"japanese":2,"english":1,"lastUsed":"2026-10-18T00:00:00Z"},
            "ka":{"japanese":9},"UPPER":{"english":9},"toolongkey":{"japanese":9},"null":null
        }}))
        .unwrap(),
    )
    .unwrap();
    let state = store.load().unwrap();
    assert_eq!(state.prefix_model.prefixes.len(), 1);
    assert_eq!(state.prefix_model.prefixes["konn"].japanese, 2);
    control::execute(
        &store,
        &json!({"op":"prefix_learn","letters":"konnichiwa","outcome":"JapaneseAccepted"}),
    )
    .unwrap();
    assert_eq!(
        store.load().unwrap().prefix_model.prefixes["konnic"].japanese,
        1
    );
    control::execute(&store, &json!({"op":"learning_clear"})).unwrap();
    assert!(store.load().unwrap().prefix_model.prefixes.is_empty());
    assert!(dir.path().join("model.json").exists());
}

#[test]
fn invalid_legacy_model_preserves_bytes_and_other_state() {
    for bytes in [b"broken".as_slice(), b"null", b"[]", b"23"] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        std::fs::write(dir.path().join("model.json"), bytes).unwrap();
        std::fs::write(
            dir.path().join("state.json"),
            br#"{"snippets":{"test":"kept"}}"#,
        )
        .unwrap();
        let state = store.load().unwrap();
        assert!(state.prefix_model.prefixes.is_empty());
        assert_eq!(state.snippets["test"], "kept");
        assert_eq!(
            std::fs::read(dir.path().join("model.json.broken")).unwrap(),
            bytes
        );
        assert_eq!(std::fs::read(dir.path().join("model.json")).unwrap(), bytes);
    }
}

#[test]
fn concurrent_prefix_updates_survive_reload_backup_restore_and_removal() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    std::thread::scope(|scope| {
        for _ in 0..6 {
            let store = &store;
            scope.spawn(move || {
                for _ in 0..4 {
                    control::execute(
                        store,
                        &json!({"op":"prefix_learn","letters":"konn","outcome":"JapaneseAccepted"}),
                    )
                    .unwrap();
                }
            });
        }
    });
    assert_eq!(
        store.load().unwrap().prefix_model.prefixes["konn"].japanese,
        24
    );
    let backup = store.backup().unwrap();
    let other = tempfile::tempdir().unwrap();
    let restored = Store::new(other.path());
    restored.restore(&backup).unwrap();
    assert_eq!(
        restored.load().unwrap().prefix_model.prefixes["konn"].japanese,
        24
    );
    assert_eq!(
        control::execute(&restored, &json!({"op":"learning_list"})).unwrap()["prefixes"]["konn"]
            ["japanese"],
        24
    );
    control::execute(
        &restored,
        &json!({"op":"learning_remove","key":"prefix:konn"}),
    )
    .unwrap();
    assert!(restored.load().unwrap().prefix_model.prefixes.is_empty());
    control::execute(&store, &json!({"op":"learning_forget_since","since":0})).unwrap();
    assert!(store.load().unwrap().prefix_model.prefixes.is_empty());
    control::execute(
        &store,
        &json!({"op":"settings_set","name":"LearningEnabled","value":false}),
    )
    .unwrap();
    assert_eq!(
        control::execute(
            &store,
            &json!({"op":"prefix_learn","letters":"konn","outcome":"JapaneseAccepted"})
        )
        .unwrap()["saved"],
        false
    );
    assert!(store.load().unwrap().prefix_model.prefixes.is_empty());
}

#[test]
fn bounded_prefixes_keep_newest_records_and_unknown_acceptance_does_not_grow_the_model() {
    let mut model = Model::default();
    let time = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    assert!(!model.learn("ka", Outcome::JapaneseAccepted, false, time));
    assert!(!model.learn("unknown", Outcome::StayedAccepted, false, time));
    assert!(key("konnichiwa") == Some("konnic"));
    let name = |mut n: usize| {
        let mut s = String::from("p");
        for _ in 0..5 {
            s.push((b'a' + (n % 26) as u8) as char);
            n /= 26;
        }
        s
    };
    for i in 0..5001 {
        model.learn(
            &name(i),
            Outcome::JapaneseAccepted,
            false,
            time + chrono::Duration::microseconds(i as i64),
        );
    }
    assert_eq!(model.prefixes.len(), 4500);
    assert!(!model.prefixes.contains_key(&name(500)));
    assert!(model.prefixes.contains_key(&name(501)));
    assert!(model.prefixes.contains_key(&name(5000)));
}
