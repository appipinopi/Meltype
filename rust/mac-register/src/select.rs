// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
fn main() {
    #[cfg(target_os = "macos")]
    std::process::exit(meltype_mac_register::native_select_main());
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("This input-source selection helper requires macOS.");
        std::process::exit(1);
    }
}
