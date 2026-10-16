// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use meltype_core::{dictionary::Dictionary, Session};
fn type_text(s: &mut Session, text: &str) {
    for c in text.chars() {
        assert!(s.key(0, c as i32, 0).consumed);
    }
}
#[test]
fn mixed_language_and_protected_spans() {
    let d = Dictionary::bundled();
    for (raw, expected) in [
        ("kyouhagoogledekensaku", "きょうはgoogleでけんさく"),
        ("ashitanomeetingwotsuika", "あしたのmeetingをついか"),
        ("https://example.com/a?b=1", "https://example.com/a?b=1"),
        ("/command $skill @file.rs", "/command $skill @file.rs"),
        ("I want to go to the park", "I want to go to the park"),
        ("日本語😀", "日本語😀"),
    ] {
        assert_eq!(d.mixed(raw, true), expected, "{raw}");
    }
}
#[test]
fn conversion_candidate_and_learning() {
    let mut s = Session::default();
    type_text(&mut s, "hashi");
    let result = s.key(0x20, 32, 0);
    let view = result.view.unwrap();
    assert!(view.converting);
    assert!(view.candidates.contains(&"橋".to_owned()));
    let index = view.candidates.iter().position(|c| c == "箸").unwrap();
    assert_eq!(s.select(index).commits[0].text, "箸");
    type_text(&mut s, "hashi");
    assert_eq!(s.key(0x20, 32, 0).view.unwrap().text, "箸");
}
#[test]
fn shortcuts_commit_once_and_pass_through() {
    let mut s = Session::default();
    type_text(&mut s, "nihongo");
    let result = s.key(0x43, 99, 2);
    assert!(!result.consumed);
    assert_eq!(result.commits.len(), 1);
    assert_eq!(result.commits[0].text, "にほんご");
    assert!(s.commit().commits.is_empty());
}
#[test]
fn edit_cancel_and_function_keys() {
    let mut s = Session::default();
    type_text(&mut s, "kya");
    s.key(8, 0, 0);
    assert_eq!(s.display(false), "ky");
    s.key(0, 97, 0);
    assert_eq!(s.key(0x76, 0, 0).view.unwrap().text, "キャ");
    assert_eq!(s.key(13, 0, 0).commits[0].text, "キャ");
    type_text(&mut s, "test");
    s.key(27, 0, 0);
    assert!(s.commit().commits.is_empty());
}
#[test]
fn invalid_unicode_leaves_composition_intact() {
    let mut s = Session::default();
    type_text(&mut s, "ka");
    for scalar in [-1, 0xd800, 0x110000] {
        assert!(!s.key(0, scalar, 0).consumed);
        assert_eq!(s.raw(), "ka");
    }
}
#[test]
fn private_input_never_learns_or_captures() {
    let mut s = Session::default();
    s.private = true;
    assert!(!s.key(0, 97, 0).consumed);
    assert!(s.preferences.english.is_empty());
}
#[test]
fn user_dictionary_is_used() {
    let mut s = Session::default();
    s.preferences
        .user_dictionary
        .insert("めると".into(), vec!["Meltype".into()]);
    type_text(&mut s, "meruto");
    assert_eq!(s.key(32, 32, 0).view.unwrap().text, "Meltype");
}
#[test]
fn typing_after_conversion_keeps_commit() {
    let mut s = Session::default();
    type_text(&mut s, "hashi");
    s.key(32, 32, 0);
    let r = s.key(0, 97, 0);
    assert_eq!(r.commits.len(), 1);
    assert_eq!(r.view.unwrap().text, "a");
}
