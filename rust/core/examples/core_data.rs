// SPDX-License-Identifier: GPL-3.0-or-later
//! Isolated upstream data / public-session interoperability host.
use meltype_core::{
    control,
    linguistics::Linguistics,
    storage::{Change, Store},
    Session,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let store = Store::new(&args[2]);
    fs::create_dir_all(&store.directory)?;
    match args[1].as_str() {
        "apps" => {
            let rows: Vec<Value> = serde_json::from_slice(&fs::read(&args[3])?)?;
            for row in rows {
                let settings =
                    meltype_core::settings::Settings::parse(&row["settings"].to_string())?;
                let mut policy = settings.app_policy(
                    row["process"].as_str().unwrap(),
                    row["looksLikeGame"].as_bool().unwrap(),
                );
                policy["name"] = row["name"].clone();
                println!("{policy}");
            }
        }
        "settings" => {
            let rows: Vec<Value> = serde_json::from_slice(&fs::read(&args[3])?)?;
            for row in rows {
                let mut settings =
                    meltype_core::settings::Settings::parse(&row["config"].to_string())?;
                let mut trace = vec![json!({"accepted":true,"settings":settings})];
                if let Some(ops) = row["ops"].as_array() {
                    for op in ops {
                        let name = op["name"].as_str().unwrap();
                        let accepted = match op["op"].as_str().unwrap() {
                            "add" => settings.add_profile(name),
                            "switch" => settings.switch_profile(name),
                            "rename" => {
                                settings.rename_profile(name, op["newName"].as_str().unwrap())
                            }
                            "remove" => settings.remove_profile(name),
                            _ => return Err("unknown profile operation".into()),
                        }
                        .is_ok();
                        trace.push(json!({"accepted":accepted,"settings":settings}));
                    }
                }
                println!("{}", json!({"name":row["name"],"expected":trace}));
            }
        }
        "backup" => fs::write(&args[3], store.backup()?)?,
        "restore" => {
            println!("{}", store.restore(&fs::read(&args[3])?)?);
        }
        "mutate" => {
            store.record(vec![
                Change::Language {
                    raw: "sushi".into(),
                    english: false,
                    explicit: true,
                },
                Change::Conversion {
                    reading: "はし".into(),
                    text: "端".into(),
                    translation: false,
                },
                Change::Conversion {
                    reading: "き".into(),
                    text: "wood".into(),
                    translation: true,
                },
                Change::Phrase {
                    reading: "きょうのよてい".into(),
                    text: "今日の予定".into(),
                },
            ])?;
            control::execute(
                &store,
                &json!({"op":"dictionary_set","reading":"はし","word":"端"}),
            )?;
            control::execute(
                &store,
                &json!({"op":"snippet_set","name":"sig","text":"更新\nRust 😀"}),
            )?;
        }
        "snapshot" => {
            let state = store.load()?;
            let language: BTreeMap<_, _> = state
                .language_counts
                .iter()
                .map(|(k, e)| {
                    (
                        k,
                        json!({"english":e.english,"count":e.count.max(1),"active":e.active(k)}),
                    )
                })
                .collect();
            let predictions: BTreeMap<_, _> = ["きょ", "きょう", "よて"]
                .into_iter()
                .map(|prefix| {
                    let mut phrases: Vec<_> = state
                        .phrases
                        .iter()
                        .filter(|p| p.reading.starts_with(prefix) && p.reading != prefix)
                        .collect();
                    phrases
                        .sort_by_key(|p| (std::cmp::Reverse(p.count), std::cmp::Reverse(p.used)));
                    (
                        prefix,
                        phrases
                            .into_iter()
                            .map(|p| p.text.clone())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            let prefixes: BTreeMap<_, _> = state
                .prefix_model
                .prefixes
                .iter()
                .map(|(k, s)| (k, json!({"japanese":s.japanese,"english":s.english})))
                .collect();
            println!(
                "{}",
                json!({"language":language,"conversions":state.preferences.selected,"translations":state.translations,"dictionary":state.preferences.user_dictionary,"predictions":predictions,"phraseCount":state.phrases.len(),"snippets":state.snippets,"prefixes":prefixes})
            );
        }
        "keys" => {
            let rows: Vec<Value> = serde_json::from_slice(&fs::read(&args[3])?)?;
            for (index, row) in rows.iter().enumerate() {
                let profile = store.directory.join(index.to_string());
                fs::create_dir_all(&profile)?;
                fs::write(
                    profile.join("config.json"),
                    serde_json::to_vec(&row["settings"])?,
                )?;
                if let Some(files) = row["files"].as_object() {
                    for (name, text) in files {
                        if Path::new(name).is_absolute()
                            || Path::new(name)
                                .components()
                                .any(|c| !matches!(c, std::path::Component::Normal(_)))
                        {
                            return Err("unsafe fixture path".into());
                        }
                        let path = profile.join(name);
                        fs::create_dir_all(path.parent().unwrap())?;
                        fs::write(path, text.as_str().ok_or("invalid file")?)?;
                    }
                }
                let mut session = Session::open(Store::new(profile))?;
                session.can_delete = row["canDelete"].as_bool().unwrap();
                let mut responses = vec![];
                for key in row["keys"].as_array().unwrap() {
                    session.set_context(row["before"].as_str(), row["after"].as_str());
                    responses.push(session.key(
                        key[0].as_i64().unwrap() as i32,
                        key[1].as_i64().unwrap() as i32,
                        key[2].as_i64().unwrap() as i32,
                    ));
                }
                println!("{}", json!({"name":row["name"],"expected":responses}));
            }
        }
        "dictionary_lookup" => {
            let s = store.load()?;
            println!("{}", serde_json::to_value(s.preferences.user_dictionary)?);
        }
        "predict" => {
            let s = store.load()?;
            println!("{}", json!(Linguistics::bundled().predict(&args[3], &s)));
        }
        _ => return Err("unknown operation".into()),
    }
    Ok(())
}
