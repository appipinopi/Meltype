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
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        writeln!(calls, "{line}").unwrap();
        calls.flush().unwrap();
        let fields: Vec<_> = line.split('\t').collect();
        let reading = fields.get(2).copied().unwrap_or("");
        if profile.join("fixture-malformed").is_file() {
            writeln!(output, "WRONG\x1fBROKEN").unwrap();
        } else if reading == "から" {
            writeln!(output).unwrap();
        } else {
            writeln!(output, "{reading}\x1fMOZC:{reading}\x1fALT:{reading}").unwrap();
        }
        output.flush().unwrap();
    }
}
