// SPDX-License-Identifier: GPL-3.0-or-later
//! Regressions discovered by expanding Cartesian batches to all twenty scenes.
use meltype_core::{storage::Store, Session};
use serde_json::{json, Value};
use std::fs;

#[test]
fn v130_key_operations_match_original() {
    let fixture: Value =
        serde_json::from_str(include_str!("data/upstream-v130-sessions.json")).unwrap();
    let mut failures = Vec::new();
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
            if key[0] == 32 {
                session.set_github_title(case["githubTitle"].as_str().map(str::to_owned));
            }
            let conversion = if key[0] == 32 && !session.is_converting() {
                let reading = session.conversion_reading();
                case["converter"]["clauses"][&reading]
                    .as_array()
                    .map(|parts| {
                        parts
                            .iter()
                            .map(|p| {
                                meltype_core::session::Clause::new(
                                    p[0].as_str().unwrap().into(),
                                    vec![p[1].as_str().unwrap().into()],
                                )
                            })
                            .collect()
                    })
            } else {
                None
            };
            let reply = if key[0] == -1 {
                session.select(key[1].as_u64().unwrap() as usize)
            } else if let Some(clauses) = conversion {
                assert!(session.begin_clauses(clauses));
                session.result(true)
            } else {
                session.key(
                    key[0].as_i64().unwrap() as i32,
                    key[1].as_i64().unwrap() as i32,
                    key[2].as_i64().unwrap() as i32,
                )
            };
            let serialized = serde_json::to_value(&reply).unwrap();
            let mut view = if reply.view.is_some() {
                let mut view = serialized["view"].clone();
                view.as_object_mut()
                    .unwrap()
                    .retain(|key, _| expected["view"].get(key).is_some());
                view
            } else {
                Value::Null
            };
            let mut wanted = expected.clone();
            // Key-operation test: UI hint/index/typed metadata is audited separately.
            if let Some(v) = wanted["view"].as_object_mut() {
                for key in [
                    "committed",
                    "hint",
                    "selectedIndex",
                    "selectedClause",
                    "typed",
                ] {
                    v.remove(key);
                }
            }
            if let Some(v) = view.as_object_mut() {
                for key in [
                    "committed",
                    "hint",
                    "selectedIndex",
                    "selectedClause",
                    "typed",
                ] {
                    v.remove(key);
                }
            }
            let actual = json!({"consumed":reply.consumed,"commits":reply.commits,"view":view});
            if actual != wanted {
                failures.push(format!(
                    "{} key {index}: actual={actual} expected={wanted}",
                    case["name"]
                ));
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
