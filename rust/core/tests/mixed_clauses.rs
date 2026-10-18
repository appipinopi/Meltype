// SPDX-License-Identifier: GPL-3.0-or-later
use meltype_core::{session::Clause, Session};
use serde_json::json;
fn typed(raw: &str) -> Session {
    let mut s = Session::default();
    s.external_converter_only = true;
    for setting in ["LiveConversion", "LearningEnabled", "PredictiveCandidates"] {
        s.settings.set(setting, json!(false)).unwrap();
    }
    for c in raw.chars() {
        s.key(
            c.to_ascii_uppercase() as i32,
            c as i32,
            i32::from(c.is_ascii_uppercase()),
        );
    }
    s
}
fn mixed(raw: &str, next: &str) -> Session {
    let mut s = typed(raw);
    let mut clauses = vec![
        Clause::new("この".into(), vec!["この".into()]),
        Clause::literal("team".into(), true),
    ];
    if !next.is_empty() {
        clauses.push(if next == "Xyz" {
            Clause::literal(next.into(), true)
        } else {
            Clause::new(next.into(), vec![next.into()])
        });
    }
    assert!(s.begin_clauses(clauses));
    s
}
#[test]
fn grows_into_english_using_its_kana_and_preserves_total_reading() {
    let mut s = mixed("konoteamdeyaru", "でやる");
    assert_eq!(s.conversion_reading(), "このteamでやる");
    s.key(0x27, 0, 1);
    assert_eq!(s.clause_readings(), ["このて", "あm", "でやる"]);
    s.key(0x27, 0, 1);
    assert_eq!(s.clause_readings(), ["このてあ", "m", "でやる"]);
}
#[test]
fn shrinking_english_moves_its_kana_into_the_next_japanese_clause() {
    let mut s = mixed("konoteamdeyaru", "でやる");
    s.key(0x27, 0, 0);
    s.key(0x25, 0, 1);
    assert_eq!(s.clause_readings(), ["この", "てあ", "mでやる"]);
    s.key(0x25, 0, 1);
    assert_eq!(s.clause_readings(), ["この", "て", "あmでやる"]);
}
#[test]
fn shrinking_before_english_inserts_a_clause_without_losing_its_selection() {
    let mut s = typed("konoteamdeyaru");
    let mut english = Clause::literal("team".into(), true);
    english.candidates.push("TEAM".into());
    english.selected = 1;
    assert!(s.begin_clauses(vec![
        Clause::new("この".into(), vec![]),
        english,
        Clause::new("でやる".into(), vec![])
    ]));
    s.key(0x25, 0, 1);
    assert_eq!(
        s.result(false).view.unwrap().clauses,
        ["こ", "の", "TEAM", "でやる"]
    );
    s.key(0x27, 0, 1);
    assert_eq!(
        s.result(false).view.unwrap().clauses,
        ["この", "TEAM", "でやる"]
    );
}
#[test]
fn unreadable_next_last_and_protected_clauses_reject_resizing_without_changes() {
    for (raw, next) in [("konoteam", ""), ("konoteamXyz", "Xyz")] {
        let mut s = mixed(raw, next);
        s.key(0x27, 0, 0);
        let before = s.result(false).view.unwrap();
        let after = s.key(0x27, 0, 1).view.unwrap();
        assert_eq!(before.text, after.text);
        assert_eq!(before.candidates, after.candidates);
        assert_eq!(before.selected_index, after.selected_index);
    }
    let raw = "https://example.com/";
    let mut s = typed(raw);
    assert!(s.begin_clauses(vec![Clause::literal(raw.into(), false)]));
    let readings = s.clause_readings();
    s.key(0x25, 0, 1);
    assert_eq!(s.display(false), raw);
    assert!(s.refresh_clause_candidates(
        &readings,
        vec![(0, vec!["should not replace protected text".into()])]
    ));
    assert_eq!(s.display(false), raw);
}
