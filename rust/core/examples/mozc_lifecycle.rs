// SPDX-License-Identifier: GPL-3.0-or-later
//! Controlled lifecycle/protocol comparison; this does not register an IME.
use meltype_core::mozc::{LearningClause, Mozc};
use std::{error::Error, path::Path};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let actions: serde_json::Value = serde_json::from_slice(&std::fs::read(&args[3])?)?;
    {
        let mut converter = Mozc::start(Path::new(&args[1]), Some(Path::new(&args[2])))?;
        for action in actions.as_array().ok_or("expected actions")? {
            let clauses: Vec<LearningClause> = serde_json::from_value(action["clauses"].clone())?;
            converter.learn(action["context"].as_str().unwrap_or(""), &clauses)?;
        }
    }
    let trace = std::fs::read_to_string(Path::new(&args[2]).join("fixture-calls.txt"))?;
    let trace: Vec<_> = trace
        .lines()
        .filter(|line| !line.starts_with("C\t"))
        .collect();
    println!("{}", serde_json::json!({"trace":trace}));
    Ok(())
}
