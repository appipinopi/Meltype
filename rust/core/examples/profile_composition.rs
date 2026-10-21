// SPDX-License-Identifier: GPL-3.0-or-later
//! Component timings for the same growing inputs as the Windows benchmark.
use meltype_core::{dictionary::Dictionary, linguistics::Linguistics};
use serde::Deserialize;
use std::{hint::black_box, time::Instant};

#[derive(Deserialize)]
struct Case {
    name: String,
    keys: Vec<[i32; 3]>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let cases: Vec<Case> = serde_json::from_str(&std::fs::read_to_string(&args[1])?)?;
    let dictionary = Dictionary::bundled();
    let linguistics = Linguistics::bundled();
    for case in cases
        .iter()
        .filter(|c| args.get(2).is_none_or(|s| s == &c.name))
    {
        let mut raw = String::new();
        let mut detection = 0.0;
        let mut spelling = 0.0;
        for [vk, ch, _] in &case.keys {
            if *vk == 13 || *vk == 8 || *ch == 0 {
                continue;
            }
            raw.push(char::from_u32(*ch as u32).unwrap());
            let started = Instant::now();
            let display = dictionary.composition(&raw, false, "", "", "Balanced");
            detection += started.elapsed().as_secs_f64() * 1000.0;
            let started = Instant::now();
            black_box(linguistics.find_misspelling(&display));
            spelling += started.elapsed().as_secs_f64() * 1000.0;
            black_box(display);
        }
        println!(
            "{}",
            serde_json::json!({"case":case.name,
            "detection_ms":detection,"misspelling_ms":spelling})
        );
    }
    Ok(())
}
