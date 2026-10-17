// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Test-only framed worker: controlled stalls/errors/descendants, no real IME.
use std::{
    io::{self, Read, Write},
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    time::Duration,
};
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--descendant") {
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    }
    assert_eq!(
        std::env::args().nth(1).as_deref(),
        Some("--stdio-tip-worker")
    );
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut serial = 0;
    loop {
        let mut size = [0; 4];
        if input.read_exact(&mut size).is_err() {
            break;
        }
        let mut bytes = vec![0; u32::from_le_bytes(size) as usize];
        input.read_exact(&mut bytes).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        if let Some(reply_file) = std::env::var_os("MELTYPE_TEST_HELLO_REPLY_FILE") {
            if let Some(request_file) = std::env::var_os("MELTYPE_TEST_HELLO_REQUEST_FILE") {
                std::fs::write(request_file, &text).unwrap();
            }
            let response = std::fs::read(reply_file).unwrap();
            output
                .write_all(&(response.len() as u32).to_le_bytes())
                .unwrap();
            output.write_all(&response).unwrap();
            output.flush().unwrap();
            continue;
        }
        let parts = text.split('|').collect::<Vec<_>>();
        serial += 1;
        if parts[0] == "exit" {
            std::process::exit(1);
        }
        if parts[0] == "oversize" {
            output
                .write_all(&(1024 * 1024 + 1u32).to_le_bytes())
                .unwrap();
            output.flush().unwrap();
            continue;
        }
        if parts[0] == "invalid" {
            output.write_all(&1u32.to_le_bytes()).unwrap();
            output.write_all(&[255]).unwrap();
            output.flush().unwrap();
            continue;
        }
        if parts[0] == "delay" {
            std::fs::write(parts[2], std::process::id().to_string()).unwrap();
            std::thread::sleep(Duration::from_millis(parts[1].parse().unwrap()));
        }
        let child = if parts[0] == "child" {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .arg("--descendant")
                .creation_flags(0x08000000)
                .stdin(Stdio::null())
                .spawn()
                .unwrap();
            let id = child.id();
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            id
        } else {
            0
        };
        let response = format!(
            "{{\"pid\":{},\"serial\":{},\"child\":{child}}}",
            std::process::id(),
            serial
        );
        output
            .write_all(&(response.len() as u32).to_le_bytes())
            .unwrap();
        output.write_all(response.as_bytes()).unwrap();
        output.flush().unwrap();
        if parts[0] == "stopreading" {
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        }
    }
}
