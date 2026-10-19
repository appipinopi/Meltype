// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use meltype_core::{session::Clause, Session};
fn mac() -> Session {
    let mut session = Session::default();
    session.external_converter_only = true;
    session.automatic_english_spacing = true;
    session
}

fn type_text(session: &mut Session, text: &str) {
    for ch in text.chars() {
        session.key(
            ch.to_ascii_uppercase() as i32,
            ch as i32,
            if ch.is_ascii_uppercase() { 1 } else { 0 },
        );
    }
}
#[test]
fn external_mixed_reading_keeps_english_and_accepts_complete_live_clauses() {
    let mut session = mac();
    type_text(&mut session, "kyouhagoogledekensaku");
    let reading = "きょうはgoogleでけんさく";
    assert_eq!(session.reading(), reading);
    assert_eq!(session.live_reading().as_deref(), Some(reading));
    let clauses = vec![
        Clause::new("きょうは".into(), vec!["今日は".into()]),
        Clause::new("google".into(), vec!["google".into()]),
        Clause::new("でけんさく".into(), vec!["で検索".into()]),
    ];
    assert!(session.update_live_conversion(clauses));
    assert_eq!(
        session.result(true).view.unwrap().text,
        "今日はgoogleで検索"
    );
    assert_eq!(session.commit().commits[0].text, "今日はgoogleで検索");
}
#[test]
fn mac_phrase_spacing_preserves_case_and_explicit_or_external_text() {
    let mut session = mac();
    type_text(&mut session, "seeyouagain");
    assert_eq!(session.commit().commits[0].text, "see you again");
    type_text(&mut session, "seeyouagain");
    session.key(0x79, 0, 0);
    assert_eq!(session.commit().commits[0].text, "SEEYOUAGAIN");
    type_text(&mut session, "seeyouagain");
    assert_eq!(
        session.commit_external(false, true).commits[0].text,
        "seeyouagain"
    );
    let mut standard = Session::default();
    type_text(&mut standard, "seeyouagain");
    assert_eq!(standard.commit().commits[0].text, "seeyouagain");
}
#[test]
fn upstream_go_preview_selects_without_committing_on_mac() {
    let mut session = mac();
    type_text(&mut session, "go");
    let preview = session.result(true).view.unwrap();
    assert_eq!(preview.candidates, ["go", "ご"]);
    assert!(!preview.converting);
    let bad = session.select_composing(2);
    assert!(!bad.consumed);
    assert!(!session.is_converting());
    let selected = session.select_composing(0);
    assert!(selected.commits.is_empty());
    assert_eq!(selected.view.unwrap().text, "go");
    assert_eq!(session.commit().commits[0].text, "go");
    type_text(&mut session, "go");
    let selected = session.select_composing(1);
    assert!(selected.commits.is_empty());
    assert_eq!(selected.view.unwrap().text, "ご");
    assert_eq!(session.commit().commits[0].text, "ご");
    let mut standard = Session::default();
    type_text(&mut standard, "go");
    assert_eq!(standard.select(1).commits[0].text, "ご");
}
#[test]
fn mac_single_clause_click_is_not_a_commit() {
    let mut session = mac();
    type_text(&mut session, "hashi");
    assert!(session.begin_clauses(vec![Clause::new(
        "はし".into(),
        vec!["橋".into(), "箸".into()]
    )]));
    let selected = session.select_composing(1);
    assert!(selected.commits.is_empty());
    assert_eq!(selected.view.unwrap().text, "箸");
    assert_eq!(session.commit().commits[0].text, "箸");
}
