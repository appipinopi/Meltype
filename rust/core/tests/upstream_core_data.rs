// SPDX-License-Identifier: GPL-3.0-or-later
//! Expected replies come only from the unmodified upstream public APIs.
use meltype_core::{settings::Settings, storage::Store, Session};
use serde_json::{json, Value};
use std::fs;

fn fixture() -> Value {
    serde_json::from_str(include_str!("data/upstream-core-data.json")).unwrap()
}

#[test]
fn original_levels_context_learning_custom_tables_and_retained_edits() {
    let fixture = fixture();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 371);
    for case in fixture["cases"].as_array().unwrap() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("config.json"), case["settings"].to_string()).unwrap();
        if let Some(files) = case["files"].as_object() {
            for (name, text) in files {
                let path = dir.path().join(name);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, text.as_str().unwrap()).unwrap();
            }
        }
        let mut session = Session::open(Store::new(dir.path())).unwrap();
        session.can_delete = case["canDelete"].as_bool().unwrap();
        let keys = case["keys"].as_array().unwrap();
        let expected = case["expected"].as_array().unwrap();
        assert_eq!(keys.len(), expected.len());
        for (index, (key, want)) in keys.iter().zip(expected).enumerate() {
            session.set_context(case["before"].as_str(), case["after"].as_str());
            let result = session.key(
                key[0].as_i64().unwrap() as i32,
                key[1].as_i64().unwrap() as i32,
                key[2].as_i64().unwrap() as i32,
            );
            let view = result.view.map(
                |v| json!({"text":v.text,"converting":v.converting,"suggestion":v.suggestion}),
            );
            assert_eq!(
                json!({"consumed":result.consumed,"commits":result.commits,"view":view}),
                *want,
                "{} key {index}",
                case["name"]
            );
        }
    }
}

fn known_settings(settings: &Settings, baseline: &Value) -> Value {
    let mut value = serde_json::to_value(settings).unwrap();
    let known = baseline.as_object().unwrap();
    value
        .as_object_mut()
        .unwrap()
        .retain(|key, _| known.contains_key(key));
    for profile in value["Profiles"].as_array_mut().unwrap() {
        if let Some(values) = profile["Values"].as_object_mut() {
            values.retain(|key, _| known.contains_key(key));
        }
    }
    value
}

#[test]
fn original_known_settings_normalization_and_profile_lifecycle() {
    let fixture = fixture();
    assert_eq!(fixture["settings"].as_array().unwrap().len(), 120);
    for case in fixture["settings"].as_array().unwrap() {
        let mut settings = Settings::parse(&case["config"].to_string()).unwrap();
        let mut trace = vec![
            json!({"accepted":true,"settings":known_settings(&settings, &fixture["settings"][0]["expected"][0]["settings"])}),
        ];
        if let Some(ops) = case["ops"].as_array() {
            for op in ops {
                let name = op["name"].as_str().unwrap();
                let accepted = match op["op"].as_str().unwrap() {
                    "add" => settings.add_profile(name),
                    "switch" => settings.switch_profile(name),
                    "rename" => settings.rename_profile(name, op["newName"].as_str().unwrap()),
                    "remove" => settings.remove_profile(name),
                    _ => panic!("unknown profile operation"),
                }
                .is_ok();
                trace.push(json!({"accepted":accepted,"settings":known_settings(&settings, &fixture["settings"][0]["expected"][0]["settings"])}));
            }
        }
        assert_eq!(json!(trace), case["expected"], "{}", case["name"]);
    }
}
