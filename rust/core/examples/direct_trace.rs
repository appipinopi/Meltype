// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic timed direct-input replay, paired with tools/parity/direct.
use meltype_core::Session;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Case {
    name: String,
    before: Option<String>,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
struct Step {
    vk: i32,
    ch: i32,
    mods: i32,
    time: u64,
    before: Option<String>,
    #[serde(default)]
    moved: bool,
    direct: Option<bool>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args().nth(1).ok_or("case JSON required")?;
    let cases: Vec<Case> = serde_json::from_str(&std::fs::read_to_string(input)?)?;
    for case in cases {
        let mut s = Session::default();
        s.direct_detection = true;
        s.can_delete = true;
        s.set_direct(true);
        for name in ["LearningEnabled", "LiveConversion", "PredictiveCandidates"] {
            s.settings.set(name, json!(false))?;
        }
        let mut document = case.before.unwrap_or_default();
        let mut responses = Vec::new();
        for step in case.steps {
            if let Some(before) = step.before {
                document = before;
            }
            if step.moved {
                s.caret_moved(0, false);
            }
            if let Some(direct) = step.direct {
                s.set_direct(direct);
            }
            s.set_context(Some(&document), None);
            let result = s.key_at(step.vk, step.ch, step.mods, step.time);
            for edit in &result.commits {
                if edit.delete_before > 0 {
                    let expected = edit.expect.as_deref().ok_or("missing expectation")?;
                    if !document.ends_with(expected)
                        || expected.encode_utf16().count() != edit.delete_before
                    {
                        return Err("unsafe Rust edit".into());
                    }
                    document.truncate(document.len() - expected.len());
                }
                document.push_str(&edit.text);
            }
            if !result.consumed && step.ch >= 32 {
                document.push(char::from_u32(step.ch as u32).ok_or("bad scalar")?);
            }
            responses.push(json!({"response":result,"direct":s.direct,"english":s.direct_english_word,"document":document}));
        }
        println!("{}", json!({"name":case.name,"responses":responses}));
    }
    Ok(())
}
