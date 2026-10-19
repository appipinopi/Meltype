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
    assert_eq!(s.display(false), "");
    type_text(&mut s, "kya");
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
    assert_eq!(r.view.unwrap().text, "あ");
}

#[test]
fn segmented_conversion_selection_resize_cancel_and_learning() {
    use meltype_core::session::Clause;
    let mut s = Session::default();
    type_text(&mut s, "hashiwotoru");
    assert!(!s.begin_clauses(vec![Clause::new("ちがう".into(), vec!["違う".into()])]));
    assert!(s.begin_clauses(vec![
        Clause::new("はしを".into(), vec!["橋を".into(), "箸を".into()]),
        Clause::new("とる".into(), vec!["取る".into(), "撮る".into()]),
    ]));
    assert_eq!(s.key(0x28, 0, 0).view.unwrap().text, "箸を取る");
    let view = s.key(0x27, 0, 0).view.unwrap();
    assert_eq!(view.selected_clause, 1);
    assert_eq!(view.clauses, ["箸を", "取る"]);
    assert!(s.select(1).commits.is_empty());
    assert_eq!(s.display(false), "箸を撮る");
    assert_eq!(s.commit().commits[0].text, "箸を撮る");
    assert_eq!(s.preferences.selected["はしを"], "箸を");
    type_text(&mut s, "hashi");
    s.key(0x20, 32, 0);
    let view = s.key(0x25, 0, 1).view.unwrap();
    assert_eq!(view.clauses.len(), 2);
    assert_eq!(view.clauses.concat(), view.text);
    let view = s.key(0x27, 0, 1).view.unwrap();
    assert_eq!(view.clauses.len(), 1);
    let view = s.key(0x1b, 0, 0).view.unwrap();
    assert!(!view.converting);
    assert!(view.clauses.is_empty());
    assert_eq!(view.text, "はし");
}

#[test]
fn undo_requires_capability_and_matching_surrounding_text() {
    let mut s = Session::default();
    type_text(&mut s, "hashi");
    s.key(0x20, 32, 0);
    let text = s.commit().commits[0].text.clone();
    assert!(!s.undo_commit().consumed);
    s.can_delete = true;
    s.set_context(Some(&format!("prefix{text}")), None);
    let result = s.undo_commit();
    assert_eq!(result.commits[0].expect.as_deref(), Some(text.as_str()));
    assert_eq!(result.commits[0].delete_before, text.encode_utf16().count());
    assert_eq!(result.view.unwrap().text, "はし");
    s.commit();
    s.set_context(Some("different"), None);
    assert!(!s.undo_commit().consumed);
    assert!(!s.undo_commit().consumed);
    type_text(&mut s, "😀");
    s.commit();
    assert_eq!(s.undo_commit().commits[0].delete_before, 2);
    s.reset();
    assert!(!s.undo_commit().consumed);
}

#[test]
fn live_preview_retains_reading_promotes_to_clauses_and_respects_settings() {
    use meltype_core::session::Clause;
    let mut s = Session::default();
    type_text(&mut s, "hashi");
    assert!(
        s.live_reading().is_none(),
        "upstream requires four UTF-16 units"
    );
    s.reset();
    type_text(&mut s, "nihongo");
    assert_eq!(s.live_reading().as_deref(), Some("にほんご"));
    let clause = || Clause::new("にほんご".into(), vec!["日本語".into(), "日本語訳".into()]);
    assert!(s.update_live_conversion(vec![clause()]));
    assert!(!s.is_converting());
    assert_eq!(s.display(false), "日本語");
    assert_eq!(s.reading(), "にほんご");
    s.begin_conversion(vec![]);
    assert!(s.is_converting());
    assert_eq!(s.result(false).view.unwrap().clauses, ["日本語"]);
    s.select(1);
    type_text(&mut s, "nihongo");
    s.update_live_conversion(vec![clause()]);
    assert_eq!(s.display(false), "日本語訳");
    s.key(8, 0, 0);
    assert_eq!(s.display(false), "にほん");
    s.reset();
    type_text(&mut s, "hello");
    assert!(s.live_reading().is_none());
    s.reset();
    type_text(&mut s, "nihongo");
    s.settings
        .set("LiveConversion", serde_json::json!(false))
        .unwrap();
    assert!(!s.update_live_conversion(vec![clause()]));
    assert_eq!(s.display(false), "にほんご");
    s.settings
        .set("LiveConversion", serde_json::json!(true))
        .unwrap();
    s.private = true;
    assert!(s.live_reading().is_none());
}

#[test]
fn code_line_toggle_survives_enter_and_resets_on_caret_move() {
    let mut s = Session::default();
    s.code_input = true;
    s.set_context(Some("let value = "), None);
    assert!(s.code_english());
    assert!(!s.key(0, 'a' as i32, 0).consumed);
    assert!(s.set_code_line(true));
    type_text(&mut s, "nihongo");
    assert_eq!(s.commit().commits[0].text, "にほんご");
    s.caret_moved(13, false);
    assert!(!s.code_english());
    s.caret_moved(0x25, false);
    assert!(s.code_english());
    s.set_context(Some("// comment "), None);
    assert!(!s.code_english());
    assert!(!s.set_code_line(true));
    type_text(&mut s, "nihongo");
    assert_eq!(s.commit().commits[0].text, "にほんご");
}

#[test]
fn selection_reconversion_infers_reading_commits_and_cancels_without_learning() {
    let mut s = Session::default();
    let result = s.reconvert_selection("橋", None);
    assert_eq!(result.selection_expect.as_deref(), Some("橋"));
    let view = result.view.unwrap();
    assert_eq!(view.text, "橋");
    let index = view.candidates.iter().position(|c| c == "箸").unwrap();
    assert_eq!(s.select(index).commits[0].text, "箸");
    s.reconvert_selection("橋", Some("はし"));
    let state = s.state.phrases.len();
    let cancel = s.key(0x1b, 0, 0);
    assert_eq!(cancel.commits[0].text, "橋");
    assert!(cancel.view.is_none());
    assert_eq!(s.state.phrases.len(), state);
    assert!(!s.reconvert_selection("未登録の漢字列", None).consumed);
    assert!(!s.reconvert_selection("橋", Some("not a reading")).consumed);
    s.private = true;
    assert!(!s.reconvert_selection("橋", Some("はし")).consumed);
    assert!(s.raw().is_empty());
}

#[test]
fn composition_control_shortcuts_and_shift_space_match_native_key_roles() {
    let mut s = Session::default();
    type_text(&mut s, "kya");
    assert_eq!(s.key(0x49, 0, 2).view.unwrap().text, "キャ");
    assert_eq!(s.key(0x55, 0, 2).view.unwrap().text, "きゃ");
    assert_eq!(s.key(0x4f, 0, 2).view.unwrap().text, "kya");
    assert_eq!(s.key(0x50, 0, 2).view.unwrap().text, "ｋｙａ");
    assert!(s.key(0x48, 0, 2).commits.is_empty());
    assert_eq!(s.raw(), "ky");
    s.key(0, 'a' as i32, 0);
    let result = s.key(32, 32, 1);
    assert!(result.commits.is_empty());
    assert!(result.view.unwrap().converting);
    let count = s.result(false).view.unwrap().candidates.len();
    assert_eq!(s.key(32, 32, 1).view.unwrap().selected_index, count - 1);
    s.commit();
    assert_eq!(s.key(32, 32, 1).commits[0].text, "　");
    assert!(!s.key(32, 32, 0).consumed);
}
#[test]
fn host_committed_preedit_learns_displayed_text_without_finalizing_or_deleting() {
    let mut s = Session::default();
    s.can_delete = true;
    type_text(&mut s, "nihon");
    assert_eq!(s.display(false), "にほn");
    let result = s.commit_preedit();
    assert_eq!(result.commits[0].text, "にほn");
    assert_eq!(result.commits[0].delete_before, 0);
    assert!(result.commits[0].expect.is_none());
    assert!(s.raw().is_empty());
    assert!(s.can_delete);
    assert!(s
        .state
        .phrases
        .iter()
        .any(|p| p.reading == "にほん" && p.text == "にほn"));
    assert!(s.commit_preedit().commits.is_empty());
}
