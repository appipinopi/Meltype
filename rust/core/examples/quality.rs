// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Compare Rust with the upstream quality suite. Writes every mismatch instead
//! of redefining expected results to match the migration's current behavior.
use meltype_core::{session::TextEdit, Session};
use serde_json::{json, Value};
fn apply(text: &mut String, edit: TextEdit) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(expected) = edit.expect {
        if !text.ends_with(&expected) || expected.encode_utf16().count() != edit.delete_before {
            return Err("host rejected an invalid replacement".into());
        }
        text.truncate(text.len() - expected.len());
    } else if edit.delete_before != 0 {
        return Err("unguarded backward deletion".into());
    }
    text.push_str(&edit.text);
    Ok(())
}
fn run(cases: &[Value], can_delete: bool) -> Result<Value, Box<dyn std::error::Error>> {
    let mut results = vec![];
    let mut pass = 0;
    for case in cases {
        let mut session = Session::default();
        session.can_delete = can_delete;
        session.settings.set("PredictiveCandidates", json!(false))?;
        session.set_context(case["before"].as_str(), None);
        let mut actual = String::new();
        for c in case["typed"].as_str().unwrap().chars() {
            let vk = if c == ' ' { 32 } else { 0 };
            let result = session.key(vk, c as i32, 0);
            for edit in result.commits {
                apply(&mut actual, edit)?;
            }
            if !result.consumed {
                actual.push(c);
            }
        }
        for edit in session.commit().commits {
            apply(&mut actual, edit)?;
        }
        let ok = actual == case["expected"];
        if ok {
            pass += 1;
        }
        results.push(json!({"category":case["category"],"typed":case["typed"],"before":case["before"],"expected":case["expected"],"actual":actual,"ok":ok}));
    }
    Ok(json!({"canDelete":can_delete,"pass":pass,"total":results.len(),"cases":results}))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../data/upstream-typing.json"))?;
    let mut report = run(&cases, true)?;
    report["upstream"] = json!("45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46");
    report["nonDeletingHost"] = run(&cases, false)?;
    let text = serde_json::to_string_pretty(&report)?;
    if let Some(path) = std::env::args().nth(1) {
        std::fs::write(path, text)?;
    } else {
        println!("{text}");
    }
    eprintln!(
        "Upstream typing cases: {}/{}, non-deleting host: {}/{}",
        report["pass"], report["total"], report["nonDeletingHost"]["pass"], report["total"]
    );
    if std::env::args().any(|s| s == "--check")
        && (report["pass"].as_u64().unwrap() < 216
            || report["nonDeletingHost"]["pass"].as_u64().unwrap() < 215)
    {
        return Err(
            "typing compatibility regressed below the recorded migration baseline (216/216 with guarded deletion, 215/216 without)".into(),
        );
    }
    Ok(())
}
