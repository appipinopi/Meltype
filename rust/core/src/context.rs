// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Code,
    Comment,
    String,
    Prompt,
}
pub fn classify(text: &str) -> LineKind {
    let line = text.rsplit(['\r', '\n']).next().unwrap_or("");
    let prompt = line.trim_start_matches(|c: char| c.is_whitespace() || "│┃║|╎┆".contains(c));
    if let Some(tail) = prompt.strip_prefix(['>', '›']) {
        if tail.is_empty() || tail.starts_with([' ', '\u{a0}']) {
            return LineKind::Prompt;
        }
    }
    let mut quote: Option<char> = None;
    let mut block: Option<(&str, LineKind)> = None;
    let mut i = 0;
    let mut first = true;
    while i < text.len() {
        let rest = &text[i..];
        let c = rest.chars().next().unwrap();
        if let Some((close, _)) = block {
            if rest.starts_with(close) {
                i += close.len();
                block = None;
            } else {
                i += c.len_utf8();
            }
            continue;
        }
        if c == '\n' || c == '\r' {
            quote = None;
            first = true;
            i += c.len_utf8();
            continue;
        }
        if let Some(q) = quote {
            if c == '\\' {
                i += 1;
                if i < text.len() {
                    i += text[i..].chars().next().unwrap().len_utf8();
                }
                continue;
            }
            if c == q {
                quote = None;
            }
            i += c.len_utf8();
            continue;
        }
        let at_start = first && !c.is_whitespace();
        if !c.is_whitespace() {
            first = false;
        }
        if let Some((open, close, kind)) = [
            ("\"\"\"", "\"\"\"", LineKind::String),
            ("'''", "'''", LineKind::String),
            ("/*", "*/", LineKind::Comment),
            ("<!--", "-->", LineKind::Comment),
        ]
        .into_iter()
        .find(|(open, _, _)| rest.starts_with(open))
        {
            block = Some((close, kind));
            i += open.len();
            continue;
        }
        let next = rest.chars().nth(1);
        let comment = rest.starts_with("//")
            || c == '#' && next.is_none_or(|c| c.is_whitespace() || c == '#')
            || rest.starts_with("--")
                && (at_start || text[..i].ends_with(' '))
                && rest
                    .get(2..)
                    .is_some_and(|s| s.is_empty() || s.starts_with(' '))
            || at_start && c == '*' && next.is_none_or(char::is_whitespace)
            || at_start
                && rest
                    .get(..4)
                    .is_some_and(|s| s.eq_ignore_ascii_case("rem "));
        if comment {
            if let Some(end) = rest.find(['\r', '\n']) {
                i += end;
                continue;
            }
            return LineKind::Comment;
        }
        if c == '"'
            || c == '`'
            || c == '\''
                && !text[..i]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_alphanumeric)
        {
            quote = Some(c);
        }
        i += c.len_utf8();
    }
    block.map(|(_, kind)| kind).unwrap_or(if quote.is_some() {
        LineKind::String
    } else {
        LineKind::Code
    })
}
pub fn prose_title(title: &str) -> bool {
    let t = title.to_ascii_lowercase();
    [".md", ".txt", ".rst", ".adoc", ".org"]
        .iter()
        .any(|ext| t.contains(ext))
}
pub fn terminal(process: &str) -> bool {
    let p = process.to_ascii_lowercase();
    [
        "windowsterminal",
        "openconsole",
        "conhost",
        "cmd",
        "powershell",
        "pwsh",
        "wezterm-gui",
        "alacritty",
        "mintty",
        "rio",
        "terminal",
        "gnome-terminal",
        "konsole",
        "kitty",
        "wezterm",
    ]
    .contains(&p.trim_end_matches(".exe"))
}
pub fn chat_focus(name: &str) -> bool {
    let name = name.to_lowercase();
    [
        "chat",
        "チャット",
        "copilot",
        "claude",
        "codex",
        "gemini",
        "prompt",
        "プロンプト",
        "message",
        "メッセージ",
        "ask ",
        "質問",
        "composer",
        "agent",
        "エージェント",
        "cascade",
        "assistant",
        "アシスタント",
    ]
    .iter()
    .any(|s| name.contains(s))
}
