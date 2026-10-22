// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
use std::path::Path;
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
pub fn xml(executable: &Path) -> String {
    let icon = escape(&executable.with_file_name("icon.png").to_string_lossy());
    let executable = escape(&executable.to_string_lossy());
    // IBus executes this command with GLib shell parsing, not an actual shell.
    // Single quotes are escaped for that parser before the XML escaping pass.
    let command = format!(
        "'{}' --ibus",
        executable.replace("&apos;", "&apos;\\&apos;&apos;")
    );
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<component>
 <name>org.freedesktop.IBus.MeltypeFlash</name>
 <description>Meltype-flash 日本語入力</description>
 <exec>{command}</exec>
 <version>{}</version><license>GPL-3.0-or-later</license>
 <author>Yukishiro, appipinopi</author>
 <homepage>https://github.com/appipinopi/Meltype</homepage>
 <engines><engine>
  <name>meltype-flash</name><longname>Meltype-flash</longname>
  <description>日本語と英語を打ち分ける日本語入力</description>
  <language>ja</language><license>GPL-3.0-or-later</license>
  <icon>{icon}</icon>
  <author>Yukishiro, appipinopi</author><layout>default</layout><symbol>あ</symbol><rank>0</rank>
 </engine></engines>
</component>
"#,
        env!("CARGO_PKG_VERSION")
    )
}
