// SPDX-License-Identifier: GPL-3.0-or-later
use meltype_core::{session::Clause, Session};
use serde_json::json;
fn session() -> Session {
    let mut s = Session::default();
    s.external_converter_only = true;
    for setting in ["LiveConversion", "LearningEnabled", "PredictiveCandidates"] {
        s.settings.set(setting, json!(false)).unwrap();
    }
    for c in "aiueokakikukeko".chars() {
        s.key(0, c as i32, 0);
    }
    assert!(s.begin_clauses(vec![
        Clause::new("あいうえお".into(), vec!["初期左".into()]),
        Clause::new("かきくけこ".into(), vec!["初期右".into()]),
    ]));
    s
}
#[test]
fn converter_refresh_uses_new_readings_and_rejects_outdated_replies() {
    let mut s = session();
    let old = s.clause_readings();
    s.key(0x27, 0, 1);
    let current = s.clause_readings();
    assert_eq!(current, ["あいうえおか", "きくけこ"]);
    assert!(!s.refresh_clause_candidates(&old, vec![(0, vec!["古い応答".into()])]));
    assert!(s.refresh_clause_candidates(
        &current,
        vec![
            (0, vec!["新しい左".into(), "左の別候補".into()]),
            (1, vec!["新しい右".into(), "右の別候補".into()]),
        ]
    ));
    assert_eq!(s.display(false), "新しい左新しい右");
    assert_eq!(s.result(false).view.unwrap().candidates[1], "左の別候補");
    s.key(0x27, 0, 0);
    assert_eq!(s.result(false).view.unwrap().candidates[1], "右の別候補");
    s.key(0x20, 0, 0);
    assert_eq!(s.commit().commits[0].text, "新しい左右の別候補");
    assert!(!s.refresh_clause_candidates(&current, vec![(0, vec!["遅延応答".into()])]));
}
#[test]
fn absent_converter_reply_keeps_the_local_reading_and_other_clause() {
    let mut s = session();
    s.key(0x25, 0, 1);
    let expected = s.clause_readings();
    let before = s.display(false);
    assert!(s.refresh_clause_candidates(&expected, vec![(0, vec![]), (99, vec!["無効".into()])]));
    assert_eq!(s.display(false), before);
    assert_eq!(s.clause_readings(), expected);
}
