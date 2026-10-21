// SPDX-License-Identifier: GPL-3.0-or-later
//! Regressions discovered by expanding Cartesian batches to all twenty scenes.
use meltype_core::{storage::Store, Session};
use serde_json::{json, Value};
use std::fs;

#[test]
fn original_prediction_order_residual_width_and_kana_symbol() {
    let fixture: Value = serde_json::from_str(include_str!(
        "data/upstream-settings-expanded-regressions.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("config.json"),
            case["settings"].to_string(),
        )
        .unwrap();
        if let Some(files) = case["files"].as_object() {
            for (name, text) in files {
                let path = directory.path().join(name);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, text.as_str().unwrap()).unwrap();
            }
        }
        let mut session = Session::open(Store::new(directory.path())).unwrap();
        for (index, (key, expected)) in case["keys"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
            .enumerate()
        {
            session.set_context(case["before"].as_str(), case["after"].as_str());
            let reply = session.key(
                key[0].as_i64().unwrap() as i32,
                key[1].as_i64().unwrap() as i32,
                key[2].as_i64().unwrap() as i32,
            );
            let serialized = serde_json::to_value(&reply).unwrap();
            let view = if reply.view.is_some() {
                let mut view = serialized["view"].clone();
                view.as_object_mut()
                    .unwrap()
                    .retain(|key, _| expected["view"].get(key).is_some());
                view
            } else {
                Value::Null
            };
            assert_eq!(
                json!({"consumed":reply.consumed,"commits":reply.commits,"view":view}),
                *expected,
                "{} key {index}",
                case["name"]
            );
        }
    }
}
