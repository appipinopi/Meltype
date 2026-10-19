// SPDX-License-Identifier: GPL-3.0-or-later
use meltype_core::Session;
use serde_json::{json, Value};
fn projection(value: &Value) -> Value {
    let view = &value["view"];
    json!({"consumed":value["consumed"],"commits":value["commits"],"view":
        if view.is_null(){Value::Null}else{json!({"text":view["text"],"converting":view["converting"],"suggestion":view["suggestion"]})}})
}
#[test]
fn original_nonconverting_misspelling_edits_preserve_surrounding_input() {
    let fixture: Value =
        serde_json::from_str(include_str!("data/upstream-misspelling-keys.json")).unwrap();
    let mut checked = 0;
    for row in fixture["cases"].as_array().unwrap() {
        if row["expected"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["view"]["converting"] == true)
        {
            continue;
        }
        checked += 1;
        let case = &row["case"];
        let mut s = Session::default();
        for name in [
            "LiveConversion",
            "LearningEnabled",
            "PredictiveCandidates",
            "ScoredSegmentation",
        ] {
            s.settings.set(name, json!(false)).unwrap();
        }
        s.settings
            .set(
                "InputStyle",
                json!(if case["kana"] == true {
                    "Kana"
                } else {
                    "Romaji"
                }),
            )
            .unwrap();
        s.settings
            .set("DetectionLevel", case["level"].clone())
            .unwrap();
        s.settings
            .set("CorrectTypos", case["correct"].clone())
            .unwrap();
        s.settings
            .set("TabConversion", case["tab"].clone())
            .unwrap();
        for (i, (key, expected)) in row["keys"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["expected"].as_array().unwrap())
            .enumerate()
        {
            let result = s.key(
                key[0].as_i64().unwrap() as i32,
                key[1].as_i64().unwrap() as i32,
                key[2].as_i64().unwrap() as i32,
            );
            assert_eq!(
                projection(&serde_json::to_value(result).unwrap()),
                projection(expected),
                "{} key {i}",
                case["name"]
            );
        }
    }
    assert_eq!(checked, 75);
}
