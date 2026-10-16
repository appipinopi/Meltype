// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use meltype_core::Session;
use serde::Deserialize;
use std::io::{self, BufRead};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Key {
        vk: i32,
        #[serde(default)]
        ch: i32,
        #[serde(default)]
        modifiers: i32,
    },
    Commit,
    Reset,
    Select {
        index: usize,
    },
    Direct {
        enabled: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--version") {
        println!("Meltype Rust {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--convert") {
        let raw = args.get(1).ok_or("--convert requires text")?;
        println!(
            "{}",
            meltype_core::dictionary::Dictionary::bundled().mixed(raw, true)
        );
        return Ok(());
    }
    if !args.is_empty() {
        return Err(
            "usage: meltype-rs [--version | --convert TEXT]; no arguments: JSON lines on stdin"
                .into(),
        );
    }
    let mut session = Session::default();
    for line in io::stdin().lock().lines() {
        let line = line?;
        let result = match serde_json::from_str::<Request>(&line) {
            Ok(request) => {
                let result = match request {
                    Request::Key { vk, ch, modifiers } => session.key(vk, ch, modifiers),
                    Request::Commit => session.commit(),
                    Request::Reset => {
                        session.reset();
                        session.result(true)
                    }
                    Request::Select { index } => session.select(index),
                    Request::Direct { enabled } => {
                        let result = session.commit();
                        session.direct = enabled;
                        result
                    }
                };
                serde_json::to_value(result)?
            }
            Err(error) => serde_json::json!({"error": error.to_string()}),
        };
        println!("{}", result);
    }
    Ok(())
}
