// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use meltype_core::{
    context::{self, LineKind},
    control,
    linguistics::Linguistics,
    protection,
    settings::Settings,
    storage::{Change, Store},
    user_dictionary, Session,
};
use serde_json::json;
fn type_text(s: &mut Session, text: &str) {
    for c in text.chars() {
        s.key(0, c as i32, 0);
    }
}
#[test]
fn settings_profiles_preserve_shared_and_future_values() {
    let mut s = Settings::parse(
        r#"{// comment
 "LiveConversion":false,"FutureSetting":{"a":1},"JapaneseThreshold":999,"SettingsVersion":5,}"#,
    )
    .unwrap();
    assert_eq!(s.integer("JapaneseThreshold"), 20);
    assert_eq!(s.0["FutureSetting"]["a"], 1);
    s.add_profile("仕事").unwrap();
    s.switch_profile("仕事").unwrap();
    s.set("SpaceAroundEnglish", json!(true)).unwrap();
    s.set("AutoUpdate", json!(false)).unwrap();
    s.switch_profile("標準").unwrap();
    assert!(!s.boolean("SpaceAroundEnglish"));
    assert!(!s.boolean("AutoUpdate"));
    s.switch_profile("仕事").unwrap();
    assert!(s.boolean("SpaceAroundEnglish"));
    assert!(s.delete_profile("仕事").is_err());
}
#[test]
fn app_rules_last_override_and_custom_kind() {
    let s=Settings::parse(r#"{"SettingsVersion":5,"AppRules":[{"Process":"Code.exe","Enabled":true},{"Process":"code.exe","Enabled":false}],"AppKinds":[]}"#).unwrap();
    assert!(!s.for_app("CODE").boolean("Enabled"));
    let s=Settings::parse(r#"{"SettingsVersion":5,"AppRules":[{"Process":"chat","Kind":"会話"}],"AppKinds":[{"Name":"会話","Base":"General","DetectionLevel":"Conservative","LiveConversion":false}]}"#).unwrap();
    assert_eq!(s.for_app("chat").text("DetectionLevel"), "Conservative");
    assert!(!s.for_app("chat").boolean("LiveConversion"));
}
#[test]
fn durable_learning_and_concurrent_updates() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::new(directory.path());
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let store = store.clone();
            std::thread::spawn(move || {
                store
                    .record(vec![Change::Conversion {
                        reading: format!("よみ{i}"),
                        text: format!("語{i}"),
                        translation: false,
                    }])
                    .unwrap()
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(store.load().unwrap().preferences.selected.len(), 8);
    let mut s = Session::open(store.clone()).unwrap();
    type_text(&mut s, "hashi");
    s.key(32, 32, 0);
    let index = s
        .result(false)
        .view
        .unwrap()
        .candidates
        .iter()
        .position(|c| c == "箸")
        .unwrap();
    s.select(index);
    let mut s = Session::open(store).unwrap();
    type_text(&mut s, "hashi");
    assert_eq!(s.key(32, 32, 0).view.unwrap().text, "箸");
}
#[test]
fn malformed_data_is_reported_and_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    std::fs::write(&path, b"broken").unwrap();
    let store = Store::new(dir.path());
    assert!(store
        .update(|s| {
            s.clear_learning();
            Ok(())
        })
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"broken");
}
#[test]
fn backup_round_trip_rejects_escapes_and_invalid_entries_before_writes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    control::execute(
        &store,
        &json!({"op":"snippet_set","name":"sig","text":"署名"}),
    )
    .unwrap();
    let bytes = store.backup().unwrap();
    store
        .update(|s| {
            s.snippets.clear();
            Ok(())
        })
        .unwrap();
    // state.json plus the seven upstream data files used by C#.
    assert_eq!(store.restore(&bytes).unwrap(), 8);
    assert_eq!(store.load().unwrap().snippets["sig"], "署名");
    let bad=serde_json::to_vec(&json!({"format":"meltype-backup","version":1,"files":{"config.json":"e30=","state.json":"!!!","../escape":"e30="}})).unwrap();
    assert!(store.restore(&bad).is_err());
    assert!(store.preview_backup(&bad).is_err());
    assert!(!dir.path().join("config.json").exists());
    for name in [
        "../escape",
        "dictionaries/../x.txt",
        "dictionaries/CON.txt",
        "dictionaries/a:b.txt",
        "C:/x",
        "dictionaries/x/y.txt",
    ] {
        assert!(!meltype_core::storage::restorable_name(name), "{name}");
    }
}
#[test]
fn backup_preview_describes_valid_targets_without_data_or_writes() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    std::fs::write(dir.path().join("config.json"), b"original settings").unwrap();
    let mut files = serde_json::Map::new();
    for (name, text) in [
        ("config.json", "{}"),
        ("state.json", "private learned content"),
        ("snippets.json", "private snippet"),
        ("userdict.txt", "private dictionary"),
        ("dictionaries/words.txt", "custom dictionary"),
    ] {
        files.insert(name.into(), json!(STANDARD.encode(text)));
    }
    files.insert("../outside".into(), json!("ignored unsupported bytes"));
    let mut backup = json!({"format":"meltype-backup","version":1,"created":"2026-10-17T12:00:00Z","files":files});
    let preview = store
        .preview_backup(&serde_json::to_vec(&backup).unwrap())
        .unwrap();
    assert_eq!(preview["fileCount"], 5);
    assert_eq!(preview["ignoredFiles"], 1);
    assert_eq!(
        preview["categories"],
        json!(["設定", "学習データ", "ユーザー辞書", "定型文"])
    );
    assert_eq!(preview["created"], "2026-10-17T12:00:00+00:00");
    assert!(preview["bytes"].as_u64().unwrap() > 0);
    assert!(preview.get("files").is_none());
    assert!(!preview.to_string().contains("private"));
    assert_eq!(
        std::fs::read(dir.path().join("config.json")).unwrap(),
        b"original settings"
    );
    assert!(!dir.path().join("state.json").exists());
    assert!(!dir.path().join("config.json.before-restore").exists());
    let combined = json!({"format":"meltype-backup","version":1,"files":{"state.json":STANDARD.encode("combined user data")}});
    let combined = store
        .preview_backup(&serde_json::to_vec(&combined).unwrap())
        .unwrap();
    assert_eq!(
        combined["categories"],
        json!(["学習データ", "ユーザー辞書", "定型文"])
    );
    backup["created"] = json!("invalid date with arbitrary content");
    assert!(store
        .preview_backup(&serde_json::to_vec(&backup).unwrap())
        .unwrap()["created"]
        .is_null());
    backup["files"]["state.json"] = json!("!!!");
    assert!(store
        .preview_backup(&serde_json::to_vec(&backup).unwrap())
        .is_err());
    assert_eq!(
        std::fs::read(dir.path().join("config.json")).unwrap(),
        b"original settings"
    );
}
#[test]
fn legacy_import_and_dictionary_encodings() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("languages.json"),
        r#"{"review":{"English":true,"Explicit":true,"Count":2,"Used":"2026-10-16T00:00:00Z"}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("conversions.json"),
        r#"{"はし":{"Text":"箸","Used":"2026-10-16T00:00:00Z"}}"#,
    )
    .unwrap();
    let state = Store::new(dir.path()).load().unwrap();
    assert_eq!(state.preferences.english.get("review"), Some(&true));
    assert_eq!(state.preferences.selected["はし"], "箸");
    let bytes = user_dictionary::export(vec![("めると".into(), "Meltype".into())].into_iter());
    let parsed = user_dictionary::parse(&bytes).unwrap();
    assert_eq!(parsed.words[0], ("めると".into(), "Meltype".into()));
    assert_eq!(parsed.encoding, "UTF-16");
    let plist=br#"<?xml version="1.0"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist><array><dict><key>shortcut</key><string>test</string><key>phrase</key><string>Test</string></dict></array></plist>"#;
    assert_eq!(
        user_dictionary::parse(plist).unwrap().words[0],
        ("test".into(), "Test".into())
    );
}
#[test]
fn context_classifier_respects_literals_blocks_and_prompts() {
    for (input, expected) in [
        ("let x = 1; // ", LineKind::Comment),
        ("let x = \"", LineKind::String),
        ("/* a\nb", LineKind::Comment),
        ("\"\"\"a\nb", LineKind::String),
        ("/* done */ x", LineKind::Code),
        ("#include", LineKind::Code),
        ("# comment", LineKind::Comment),
        ("│ › ", LineKind::Prompt),
        ("PS C:\\> ", LineKind::Code),
        ("don't", LineKind::Code),
    ] {
        assert_eq!(context::classify(input), expected, "{input}");
    }
}
#[test]
fn dictionaries_supply_context_translation_gloss_and_correction() {
    let d = Linguistics::bundled();
    assert_eq!(d.context("はしを", "ご飯を食べる"), Some("箸を".into()));
    assert!(d.meaning("箸", "はし").is_some());
    assert!(d
        .translations("複雑な", "ふくざつな")
        .contains(&"complex".into()));
    assert_eq!(d.misspelling("ぶれすれっど"), Some("ブレスレット".into()));
    assert_eq!(d.misspelling("シミュレーション"), None);
}
#[test]
fn predictions_commit_only_after_selection() {
    let mut s = Session::default();
    s.preferences
        .user_dictionary
        .insert("めるたいぷ".into(), vec!["Meltype".into()]);
    type_text(&mut s, "meru");
    assert!(s.predictions().contains(&"Meltype".into()));
    assert_eq!(s.commit().commits[0].text, "める");
    type_text(&mut s, "meru");
    s.key(9, 0, 0);
    assert_eq!(s.commit().commits[0].text, "Meltype");
}
#[test]
fn halfwidth_case_and_jis_kana() {
    let mut s = Session::default();
    type_text(&mut s, "gakkou");
    s.key(0x77, 0, 0);
    assert_eq!(s.commit().commits[0].text, "ｶﾞｯｺｳ");
    type_text(&mut s, "review");
    s.key(0x79, 0, 0);
    assert_eq!(s.display(false), "REVIEW");
    s.key(0x79, 0, 0);
    assert_eq!(s.display(false), "Review");
    s.key(0x79, 0, 0);
    assert_eq!(s.commit().commits[0].text, "review");
    s.settings.set("InputStyle", json!("Kana")).unwrap();
    for (vk, ch) in [(0x54, 't'), (0xc0, '@'), (0x51, 'q')] {
        s.key(vk, ch as i32, 0);
    }
    assert_eq!(s.commit().commits[0].text, "がた");
}
#[test]
fn snippets_tab_and_private_learning() {
    let mut s = Session::default();
    s.state
        .snippets
        .insert("sig".into(), "署名\nMeltype".into());
    type_text(&mut s, ";sig");
    assert_eq!(s.key(9, 0, 0).commits[0].text, "署名\nMeltype");
    s.private = true;
    assert!(!s.key(0, 'a' as i32, 0).consumed);
    assert!(s.raw().is_empty());
}
#[test]
fn protected_spans_preserve_automatic_text_and_honor_explicit_width() {
    let raw = "https://example.com/a 日本語 @name C:\\foo\\bar";
    let spans = protection::scan(raw);
    assert_eq!(spans.len(), 3);
    let mut automatic = Session::default();
    type_text(&mut automatic, "https://example.com/a");
    assert_eq!(automatic.commit().commits[0].text, "https://example.com/a");
    // Upstream CompositionText.Display explicitly applies width/case to Raw;
    // protection is an automatic-conversion policy, not a veto of F9/F10.
    let mut explicit = Session::default();
    type_text(&mut explicit, "https://example.com/a");
    explicit.key(0x78, 0, 0);
    assert_eq!(
        explicit.commit().commits[0].text,
        "ｈｔｔｐｓ：／／ｅｘａｍｐｌｅ．ｃｏｍ／ａ"
    );
    let mut uppercase = Session::default();
    uppercase
        .settings
        .set("SigilWordsDirect", json!(false))
        .unwrap();
    type_text(&mut uppercase, "@name");
    uppercase.key(0x79, 0, 0);
    assert_eq!(uppercase.display(false), "@NAME");
    uppercase.key(0x79, 0, 0);
    assert_eq!(uppercase.commit().commits[0].text, "@name");
}
