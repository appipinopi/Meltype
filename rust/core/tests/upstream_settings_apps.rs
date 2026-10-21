// SPDX-License-Identifier: GPL-3.0-or-later
//! Frozen replies from the unmodified C# Settings API.
use meltype_core::settings::Settings;
use serde_json::Value;
#[test]
fn original_app_rules_profiles_game_detection_and_custom_kinds() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("data/upstream-settings-apps.json")).unwrap();
    assert_eq!(rows.len(), 384);
    for row in rows {
        let settings = Settings::parse(&row["settings"].to_string()).unwrap();
        let mut actual = settings.app_policy(
            row["process"].as_str().unwrap(),
            row["looksLikeGame"].as_bool().unwrap(),
        );
        actual["name"] = row["name"].clone();
        assert_eq!(actual, row["expected"], "case {}", row["name"]);
    }
}
