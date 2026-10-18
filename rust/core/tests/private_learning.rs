// SPDX-License-Identifier: GPL-3.0-or-later
use meltype_core::session::Session;
use serde_json::json;

#[test]
fn private_scope_converts_without_learning_and_normal_scope_resumes() {
    let mut session = Session::default();
    session
        .settings
        .0
        .insert("LearningEnabled".into(), json!(true));
    let baseline = serde_json::to_value(&session.state).unwrap();
    session.no_learning = true;
    for c in "nihongo".chars() {
        assert!(
            session
                .key(c.to_ascii_uppercase() as i32, c as i32, 0)
                .consumed
        );
    }
    session.key(0x78, 0, 0);
    let committed = session.commit();
    assert!(!committed.commits.is_empty());
    assert_eq!(serde_json::to_value(&session.state).unwrap(), baseline);
    assert_eq!(
        session.control(&json!({"op":"prefix_learn"})).unwrap(),
        json!({"saved":false})
    );
    session.no_learning = false;
    for c in "nihongo".chars() {
        session.key(c.to_ascii_uppercase() as i32, c as i32, 0);
    }
    session.key(0x78, 0, 0);
    session.commit();
    assert_ne!(serde_json::to_value(&session.state).unwrap(), baseline);
}
