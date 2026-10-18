// SPDX-License-Identifier: GPL-3.0-or-later
//! Deterministic legacy-hook trace host; never sends input to the OS.
use meltype_core::{
    input_session::{Command, InputSession},
    settings::Settings,
};
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Deserialize)]
struct Case {
    name: String,
    #[serde(default)]
    settings: Value,
    steps: Vec<Value>,
    #[serde(default)]
    learn: Vec<Value>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::env::args().nth(1).ok_or("case JSON required")?;
    let cases: Vec<Case> = serde_json::from_str(&std::fs::read_to_string(file)?)?;
    for case in cases {
        let mut settings = Settings::default();
        if let Some(values) = case.settings.as_object() {
            for (key, value) in values {
                settings.set(key, value.clone())?;
            }
        }
        let mut session = InputSession::default();
        let mut model = meltype_core::prefix_learning::Model::default();
        for item in &case.learn {
            for _ in 0..item["repeat"].as_u64().unwrap_or(1) {
                model.learn(
                    item["letters"].as_str().ok_or("letters required")?,
                    serde_json::from_value(item["outcome"].clone())?,
                    item["decidedEnglish"].as_bool().unwrap_or(false),
                    chrono::Utc::now(),
                );
            }
        }
        let counts: std::collections::BTreeMap<_, _> = case
            .learn
            .iter()
            .filter_map(|item| item["letters"].as_str())
            .filter_map(meltype_core::prefix_learning::key)
            .map(|key| {
                (
                    key.to_owned(),
                    model
                        .prefixes
                        .get(key)
                        .map(|s| json!({"japanese":s.japanese,"english":s.english})),
                )
            })
            .collect();
        session.set_model(model);
        let mut responses = vec![];
        for step in case.steps {
            let command: Command = serde_json::from_value(step)?;
            responses.push(session.execute(command, &settings));
        }
        println!(
            "{}",
            json!({"name":case.name,"responses":responses,"model":counts})
        );
    }
    Ok(())
}
