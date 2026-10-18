// SPDX-License-Identifier: GPL-3.0-or-later
//! Original InputSession and UserModel evidence, including every replay event.
use meltype_core::{
    input_session::{Command, InputSession},
    prefix_learning::Model,
    settings::Settings,
};
use serde_json::{json, Value};
fn project(mut row: Value) -> Value {
    fn strip(d: &mut Value) {
        if let Some(values) = d["contributions"].as_array_mut() {
            for value in values {
                value.as_object_mut().unwrap().remove("reason");
            }
        }
    }
    if row["decision"].is_object() {
        strip(&mut row["decision"]);
    }
    for flush in row["flushes"].as_array_mut().unwrap() {
        strip(&mut flush["result"]);
    }
    row
}
#[test]
fn original_hook_and_learning_match_all_182_event_sequences() {
    let golden: Value =
        serde_json::from_str(include_str!("data/upstream-hook-session.json")).unwrap();
    let cases = golden["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 182);
    for case in cases {
        let mut settings = Settings::default();
        for (name, value) in case["settings"].as_object().unwrap() {
            settings.set(name, value.clone()).unwrap();
        }
        let mut model = Model::default();
        if let Some(records) = case["learn"].as_array() {
            for item in records {
                for _ in 0..item["repeat"].as_u64().unwrap_or(1) {
                    model.learn(
                        item["letters"].as_str().unwrap(),
                        serde_json::from_value(item["outcome"].clone()).unwrap(),
                        item["decidedEnglish"].as_bool().unwrap_or(false),
                        chrono::Utc::now(),
                    );
                }
            }
        }
        for (key, expected) in case["model"].as_object().unwrap() {
            let actual = model
                .prefixes
                .get(key)
                .map(|s| json!({"japanese":s.japanese,"english":s.english}));
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                *expected,
                "{} prefix {key}",
                case["name"]
            );
        }
        let mut session = InputSession::default();
        session.set_model(model);
        let steps = case["steps"].as_array().unwrap();
        let expected = case["expected"].as_array().unwrap();
        assert_eq!(steps.len(), expected.len());
        for (i, (step, expected)) in steps.iter().zip(expected).enumerate() {
            let command: Command = serde_json::from_value(step.clone()).unwrap();
            let out = session.execute(command, &settings);
            assert_eq!(
                project(serde_json::to_value(out).unwrap()),
                *expected,
                "{} event {i}: {step}",
                case["name"]
            );
        }
    }
}
#[test]
fn stale_workers_cannot_drain_a_newer_session_or_duplicate_a_batch() {
    let settings = Settings::default();
    let mut session = InputSession::default();
    let run = |session: &mut InputSession, value: Value| {
        session.execute(serde_json::from_value(value).unwrap(), &settings)
    };
    for (i, vk) in [75, 79, 78, 78].into_iter().enumerate() {
        run(&mut session, json!({"op":"key","vk":vk,"time":i*40}));
    }
    let aborted = run(&mut session, json!({"op":"abort"}));
    assert_eq!(aborted.events.unwrap().len(), 4);
    for (i, vk) in [75, 79, 78, 78].into_iter().enumerate() {
        run(&mut session, json!({"op":"key","vk":vk,"time":1000+i*40}));
    }
    assert!(run(&mut session, json!({"op":"take","sessionId":1}))
        .events
        .is_none());
    assert_eq!(
        run(&mut session, json!({"op":"take","sessionId":2}))
            .events
            .unwrap()
            .len(),
        4
    );
    assert!(run(&mut session, json!({"op":"take","sessionId":2}))
        .events
        .is_none());
    assert!(run(&mut session, json!({"op":"take","sessionId":2}))
        .events
        .is_none());
}
