// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
mod component;
mod keys;
#[cfg(target_os = "linux")]
mod platform;

fn main() {
    if std::env::args().any(|arg| arg == "--print-component") {
        match std::env::current_exe() {
            Ok(exe) => print!("{}", component::xml(&exe)),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--version") {
        println!("Meltype-flash IBus {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    #[cfg(target_os = "linux")]
    if let Err(error) = platform::run() {
        eprintln!("Meltype IBus: {error}");
        std::process::exit(1);
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("Meltype IBus requires Linux.");
        std::process::exit(1);
    }
}
