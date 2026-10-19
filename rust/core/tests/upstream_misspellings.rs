// SPDX-License-Identifier: GPL-3.0-or-later
//! Original dictionary results, including preserved upstream surrogate failures.
use meltype_core::linguistics::Linguistics;
use serde_json::Value;
#[test]
fn original_misspelling_dictionary_results_match() {
    let golden: Value =
        serde_json::from_str(include_str!("data/upstream-misspellings.json")).unwrap();
    let cases = golden["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2182);
    let mut errors = 0;
    for row in cases {
        let reading = row["reading"].as_str().unwrap();
        let actual = Linguistics::bundled().misspelling(reading);
        if row["error"].is_null() {
            assert_eq!(actual.as_deref(), row["expected"].as_str(), "{reading}");
        } else {
            // C# substrings can split a surrogate before FormD normalization.
            // Rust scalar-safe processing must run without reproducing a crash;
            // do not label this as a matching normal return from the original.
            assert_eq!(row["error"], "ArgumentException");
            errors += 1;
        }
    }
    assert_eq!(errors, golden["original_errors"].as_u64().unwrap());
}
