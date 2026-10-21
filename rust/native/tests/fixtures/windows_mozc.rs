// SPDX-License-Identifier: GPL-3.0-or-later
//! Private protocol host; test input is unrelated to any user's dictionary.
use std::{
    fs::OpenOptions,
    io::{self, BufRead, Write},
};
fn main() {
    let profile = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::write(
        profile.join("fixture-cwd.txt"),
        std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .as_bytes(),
    )
    .unwrap();
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetErrorMode() -> u32;
        }
        std::fs::write(
            profile.join("fixture-error-mode.txt"),
            unsafe { GetErrorMode() }.to_string(),
        )
        .unwrap();
    }
    let mut calls = OpenOptions::new()
        .create(true)
        .append(true)
        .open(profile.join("fixture-calls.txt"))
        .unwrap();
    let mut output = io::stdout().lock();
    writeln!(output, "READY").unwrap();
    output.flush().unwrap();
    let mut learned = std::collections::HashMap::<String, String>::new();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        writeln!(calls, "{line}").unwrap();
        calls.flush().unwrap();
        if line == "Q" {
            if profile.join("fixture-ignore-quit").is_file() { continue; }
            std::fs::write(profile.join("fixture-saved-on-quit"), b"saved").unwrap();
            return;
        }
        let fields: Vec<_> = line.split('\t').collect();
        let reading = fields.get(2).copied().unwrap_or("");
        if fields[0] == "L" {
            if profile.join("fixture-fail-learning").is_file() { std::process::exit(1); }
            if profile.join("fixture-slow-learning").is_file() {
                std::thread::sleep(std::time::Duration::from_millis(750));
            }
            if !reading.contains("NG") {
                for record in reading.split('\x1e') {
                    if let Some((kana, text)) = record.split_once('\x1f') {
                        if text == format!("MOZC:{kana}") || text == format!("ALT:{kana}") {
                            learned.insert(kana.into(), text.into());
                        }
                    }
                }
            }
            writeln!(output, "{}", if reading.contains("NG") { "NG" } else { "OK" }).unwrap();
        } else if fields[0] == "S" {
            writeln!(output, "{}", if profile.join("fixture-reject-save").is_file() { "NG" } else { "OK" }).unwrap();
        } else if profile.join("fixture-malformed").is_file() {
            writeln!(output, "WRONG\x1fBROKEN").unwrap();
        } else if reading == "から" {
            writeln!(output).unwrap();
        } else {
            let first = learned.get(reading).cloned().unwrap_or_else(|| format!("MOZC:{reading}"));
            writeln!(output, "{reading}\x1f{first}\x1fMOZC:{reading}\x1fALT:{reading}").unwrap();
        }
        output.flush().unwrap();
    }
}
