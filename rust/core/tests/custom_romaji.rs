// SPDX-License-Identifier: GPL-3.0-or-later
use meltype_core::{romaji::CustomTable, storage::Store, Session};
use serde_json::json;
fn session(table: &str) -> (tempfile::TempDir, Session) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("dictionaries")).unwrap();
    std::fs::write(directory.path().join("dictionaries/romaji.txt"), table).unwrap();
    let mut session = Session::open(Store::new(directory.path())).unwrap();
    for key in ["LearningEnabled", "PredictiveCandidates", "LiveConversion"] {
        session.settings.set(key, json!(false)).unwrap();
    }
    (directory, session)
}
fn type_text(session: &mut Session, text: &str) {
    for c in text.chars() {
        session.key(
            c.to_ascii_uppercase() as i32,
            c as i32,
            i32::from(c.is_ascii_uppercase()),
        );
    }
}
#[test]
fn table_parser_matches_original_validation_and_last_definition_wins() {
    let table = CustomTable::parse(" # comment\nKZ\tかん\nkq　かい\nkk きん extra\nkz\tきん\n;a か\nabc english\nabc\n許可 かな\nqp Ａ\n");
    assert_eq!(table.entries().len(), 4);
    assert_eq!(table.entries()["kz"], "きん");
    assert_eq!(table.entries()["kq"], "かい");
    assert_eq!(table.entries()["kk"], "きん");
    assert_eq!(table.entries()["qp"], "Ａ");
}
#[test]
fn custom_spellings_override_builtin_sokuon_n_and_long_spelling() {
    let (_directory, mut s) = session("kk\tきん\nkq\tかい\nnn\tの\nka\tきゃ\nqwertyu\tあ\n");
    for (raw, expected) in [
        ("kk", "きん"),
        ("kq", "かい"),
        ("nn", "の"),
        ("ka", "きゃ"),
        ("qwertyu", "あ"),
    ] {
        type_text(&mut s, raw);
        assert_eq!(s.reading(), expected, "{raw}");
        s.key(0x76, 0, 0);
        assert_eq!(
            s.commit().commits[0].text,
            meltype_core::romaji::katakana(expected)
        );
    }
}
#[test]
fn custom_unit_is_retained_for_delete_and_later_custom_prefix() {
    let (_directory, mut s) = session("kk\tきん\nn\tな\nnn\tの\n");
    type_text(&mut s, "nn");
    assert_eq!(s.reading(), "なな");
    s.key(8, 0, 0);
    assert_eq!(s.reading(), "な");
    s.reset();
    type_text(&mut s, "kk");
    assert_eq!(s.key(8, 0, 0).view.map(|v| v.text), None);
}
#[test]
fn reload_and_multiple_sessions_do_not_leak_tables() {
    let (directory, mut a) = session("kk\tきん\n");
    let (_other, mut b) = session("kk\tかい\n");
    type_text(&mut a, "kk");
    type_text(&mut b, "kk");
    assert_eq!(a.reading(), "きん");
    assert_eq!(b.reading(), "かい");
    let mut plain = Session::default();
    type_text(&mut plain, "kk");
    assert_ne!(plain.reading(), "きん");
    assert_ne!(plain.reading(), "かい");
    std::fs::write(
        directory.path().join("dictionaries/romaji.txt"),
        "kk\tくう\n",
    )
    .unwrap();
    a.reload().unwrap();
    assert_eq!(a.reading(), "きん", "active composition keeps its table");
    a.reset();
    a.reload().unwrap();
    type_text(&mut a, "kk");
    assert_eq!(a.reading(), "くう");
}

#[test]
fn undo_restores_original_units_and_table_after_a_reload() {
    let (directory, mut s) = session("n\tな\nnn\tの\n");
    s.can_delete = true;
    type_text(&mut s, "nn");
    assert_eq!(s.commit().commits[0].text, "なな");
    std::fs::write(directory.path().join("dictionaries/romaji.txt"), "nn\tに\n").unwrap();
    s.reload().unwrap();
    s.set_context(Some("なな"), None);
    let undone = s.undo_commit();
    assert_eq!(undone.commits[0].delete_before, 2);
    assert_eq!(undone.commits[0].expect.as_deref(), Some("なな"));
    assert_eq!(s.reading(), "なな");
    assert_eq!(s.commit().commits[0].text, "なな");
}

#[test]
fn utf16_table_and_failed_read_fall_back_without_losing_settings() {
    let (directory, mut s) = session("kk\tきん\n");
    let file = directory.path().join("dictionaries/romaji.txt");
    let bytes: Vec<_> = std::iter::once(0xfeffu16)
        .chain("kk\tかい\n".encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    std::fs::write(&file, bytes).unwrap();
    s.reload().unwrap();
    type_text(&mut s, "kk");
    assert_eq!(s.reading(), "かい");
    s.reset();
    std::fs::write(&file, [0xff, 0xfe, 0x61]).unwrap();
    let loaded = Session::open(Store::new(directory.path())).unwrap();
    assert!(loaded
        .storage_error
        .as_deref()
        .is_some_and(|e| e.contains("romaji.txt")));
    assert!(loaded.settings.boolean("Enabled"));
}

#[test]
fn all_custom_table_key_traces_match_the_original_csharp_host() {
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("data/upstream-custom-romaji.json")).unwrap();
    for case in golden["cases"].as_array().unwrap() {
        let (_directory, mut s) = session(golden["table"].as_str().unwrap());
        s.can_delete = true;
        for (index, (key, expected)) in case["keys"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
            .enumerate()
        {
            let r = s.key(
                key[0].as_i64().unwrap() as i32,
                key[1].as_i64().unwrap() as i32,
                key[2].as_i64().unwrap() as i32,
            );
            assert_eq!(
                json!({"consumed":r.consumed,"commits":r.commits,
                "text":r.view.as_ref().map(|v|v.text.as_str()),"converting":r.view.as_ref().is_some_and(|v|v.converting)}),
                *expected,
                "{} at key {index}",
                case["name"]
            );
        }
    }
}
