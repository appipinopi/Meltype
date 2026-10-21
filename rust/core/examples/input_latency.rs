// SPDX-License-Identifier: GPL-3.0-or-later
//! Diagnose one synthetic buffered input, reporting each key before and after.
//! This verbose host is not used for comparative performance claims.
use meltype_core::Session;
use serde_json::json;
use std::time::Instant;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = std::env::args().nth(1).ok_or("input text required")?;
    let mut session = Session::default();
    session.can_delete = true;
    for name in ["LiveConversion", "PredictiveCandidates", "LearningEnabled"] {
        session.settings.set(name, json!(false))?;
    }
    for (i, c) in text.chars().chain(std::iter::once('\n')).enumerate() {
        eprintln!("begin key {i}: {c:?}");
        let start = Instant::now();
        let result = session.key(
            if c == '\n' {
                13
            } else {
                c.to_ascii_uppercase() as i32
            },
            if c == '\n' { 0 } else { c as i32 },
            i32::from(c.is_ascii_uppercase()),
        );
        println!(
            "{}",
            json!({"index":i,"key":c,"ms":start.elapsed().as_secs_f64()*1000.0,
            "text":result.view.as_ref().map(|v|v.text.as_str()),"commits":result.commits})
        );
    }
    Ok(())
}
