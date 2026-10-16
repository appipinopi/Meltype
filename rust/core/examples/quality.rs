// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Compare Rust with the upstream quality suite. Writes every mismatch instead
//! of redefining expected results to match the migration's current behavior.
use meltype_core::Session;
use serde_json::{json, Value};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../data/upstream-typing.json"))?;
    let mut results = vec![];
    let mut pass = 0;
    for case in cases {
        let mut session = Session::default();
        session.settings.set("PredictiveCandidates", json!(false))?;
        session.set_context(case["before"].as_str(), None);
        let mut actual = String::new();
        for c in case["typed"].as_str().unwrap().chars() {
            let vk = if c == ' ' { 32 } else { 0 };
            let result = session.key(vk, c as i32, 0);
            for edit in result.commits {
                actual.push_str(&edit.text);
            }
            if !result.consumed {
                actual.push(c);
            }
        }
        for edit in session.commit().commits {
            actual.push_str(&edit.text);
        }
        let ok = actual == case["expected"];
        if ok {
            pass += 1;
        }
        results.push(json!({"category":case["category"],"typed":case["typed"],"before":case["before"],"expected":case["expected"],"actual":actual,"ok":ok}));
    }
    let report = json!({"upstream":"45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46","pass":pass,"total":results.len(),"cases":results});
    let text = serde_json::to_string_pretty(&report)?;
    if let Some(path) = std::env::args().nth(1) {
        std::fs::write(path, text)?;
    } else {
        println!("{text}");
    }
    eprintln!("Upstream typing cases: {pass}/{}", results.len());
    if std::env::args().any(|s| s == "--check") && pass < 191 {
        return Err(
            "typing compatibility regressed below the recorded migration baseline (191/216)".into(),
        );
    }
    Ok(())
}
