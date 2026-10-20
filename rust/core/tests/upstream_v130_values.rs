// SPDX-License-Identifier: GPL-3.0-or-later
use meltype_core::{calculator, number_candidates as numbers, shortcuts};
use serde_json::{json, Value};
#[test]
fn values_match_unmodified_v130_core() {
    let fixture: Value =
        serde_json::from_str(include_str!("data/upstream-v130-values.json")).unwrap();
    let mut differences = vec![];
    for case in fixture["cases"].as_array().unwrap() {
        let r = &case["request"];
        let text = r["text"].as_str().unwrap_or("");
        let value=match r["kind"].as_str().unwrap() {
            "github-normalize"=>json!(meltype_core::github_titles::normalize(text)),
            "github-valid"=>json!(meltype_core::github_titles::valid(text)),
            "names"=>json!(meltype_core::linguistics::Linguistics::bundled().predict_english_names(text)),
            "calculator"=>json!(calculator::evaluate(text)),
            "number"=>json!(numbers::candidates(text)),
            "era"=>json!(numbers::era_to_year(text)),
            "unit"=>json!(numbers::traditional_units(text)),
            "seasonal"=>json!(numbers::seasonal(r["month"].as_u64().unwrap() as u32)),
            "clipboard"=>json!(numbers::clipboard_line(text)),
            "shortcuts"=>Value::Array(shortcuts::parse_list(text).iter().map(|s| json!({"Vk":s.vk,"Ctrl":s.modifiers&2!=0,"Alt":s.modifiers&4!=0,"Shift":s.modifiers&1!=0,"Win":s.modifiers&8!=0})).collect()),
            _=>panic!("unknown fixture"),
        };
        if value != case["expected"] {
            differences.push(json!({"request":r,"expected":case["expected"],"actual":value}));
        }
    }
    assert!(differences.is_empty(), "{}", json!(differences));
}
