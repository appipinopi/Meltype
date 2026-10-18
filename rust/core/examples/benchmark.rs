// SPDX-License-Identifier: GPL-3.0-or-later
//! In-memory common-core benchmark; paired with tools/performance/windows.
use meltype_core::Session;
use serde::Deserialize;
use serde_json::json;
use std::{hint::black_box, time::Instant};

#[derive(Deserialize)]
struct Case {
    name: String,
    keys: Vec<[i32; 3]>,
    #[serde(default)]
    scored: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let cases: Vec<Case> = serde_json::from_str(&std::fs::read_to_string(&args[1])?)?;
    let samples: usize = args[2].parse()?;
    let warmups: usize = args[3].parse()?;
    let progress = std::env::var("MELTYPE_BENCH_PROGRESS").is_ok_and(|s| s == "1");
    for case in cases {
        for sample in 0..warmups + samples {
            let mut session = if let Some(directory) = args.get(4) {
                Session::open(meltype_core::storage::Store::new(directory))?
            } else {
                Session::default()
            };
            session.can_delete = true;
            session
                .settings
                .set("ScoredSegmentation", json!(case.scored))?;
            for setting in ["LiveConversion", "PredictiveCandidates", "LearningEnabled"] {
                session.settings.set(setting, json!(false))?;
            }
            let mut responses = Vec::with_capacity(case.keys.len());
            let mut keys_ms = Vec::with_capacity(case.keys.len());
            let started = Instant::now();
            for [vk, ch, modifiers] in &case.keys {
                let key_started = Instant::now();
                let result = session.key(*vk, *ch, *modifiers);
                let response = serde_json::to_string(&result)?;
                black_box(&response);
                keys_ms.push(key_started.elapsed().as_secs_f64() * 1000.0);
                responses.push(response);
            }
            let total_ms = started.elapsed().as_secs_f64() * 1000.0;
            if sample >= warmups {
                println!(
                    "{}",
                    json!({"case": case.name, "sample": sample-warmups,
                    "total_ms": total_ms, "keys_ms": keys_ms, "responses": responses})
                );
            }
        }
        if progress {
            eprintln!("Completed {}", case.name);
        }
    }
    Ok(())
}
