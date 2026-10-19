// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
use meltype_core::{
    storage::{Change, Store},
    Session,
};
use serde_json::json;

fn session(level: &str, raw: &str, before: &str, after: &str) -> Session {
    let mut s = Session::default();
    s.settings.set("DetectionLevel", json!(level)).unwrap();
    // These tests exercise protected preedit transforms, not passthrough.
    s.settings.set("SigilWordsDirect", json!(false)).unwrap();
    s.set_context(Some(before), Some(after));
    for c in raw.chars() {
        s.key(
            if c.is_ascii_alphabetic() {
                c.to_ascii_uppercase() as i32
            } else {
                0
            },
            c as i32,
            i32::from(c.is_ascii_uppercase()),
        );
    }
    s
}

#[test]
fn upstream_feedback_level_and_manual_examples_keep_their_original_expectations() {
    // FeedbackTests.Levels_ChangeHowEagerlyEnglishIsShown,
    // Manual_OnlySuggests_TabAccepts, ShortParticles_AfterEnglishWord_AreJapanese,
    // and ShortWords_InEnglishSentence_AreEnglish.
    for (level, raw, before, expected) in [
        ("Balanced", "sushi", "I like ", "sushi"),
        ("Conservative", "sushi", "I ", "すし"),
        ("Conservative", "google", "", "google"),
        ("Conservative", "amazon", "", "あまぞn"),
        ("Manual", "google", "", "ごおgぇ"),
        ("Manual", "Google", "", "Google"),
        ("Manual", "kyouhaGooglede", "", "きょうはGoogleで"),
        ("Aggressive", "no", "GitHub", "no"),
        ("Balanced", "no", "GitHub", "の"),
        ("Balanced", "to", "GitHub", "と"),
        ("Balanced", "ga", "GitHub", "が"),
        ("Balanced", "to", "I want ", "to"),
        ("Balanced", "a", "this is ", "a"),
        ("Balanced", "is", "this ", "is"),
    ] {
        let s = session(level, raw, before, "");
        assert_eq!(s.display(false), expected, "{level}: {before}|{raw}");
    }
}

#[test]
fn conservative_requires_two_sides_or_an_english_sentence_for_readable_words() {
    for (before, after, expected) in [
        ("I like ", "", "sushi"),
        ("I ", "", "すし"),
        ("I ", " please", "sushi"),
        ("I love ", "が好き", "すし"),
        ("日本語の後に", "", "すし"),
        ("I like. ", "", "すし"),
    ] {
        assert_eq!(
            session("Conservative", "sushi", before, after).display(false),
            expected,
            "{before}|sushi|{after}"
        );
    }
    assert_eq!(
        session("Conservative", "kyouhagoogledekensaku", "", "").display(false),
        "きょうはgoogleでけんさく"
    );
    assert_eq!(
        session("Conservative", "apple", "", "").display(false),
        "apple"
    );
    assert_eq!(
        session("Conservative", "no", "I ", " please").display(false),
        "の"
    );
    assert_eq!(
        session("Conservative", "no", "I want ", "").display(false),
        "no"
    );
}

#[test]
fn manual_tab_is_an_explicit_english_choice_even_with_correction_and_conversion_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::new(dir.path());
    store
        .change_settings(|s| {
            s.set("DetectionLevel", json!("Manual"))?;
            s.set("CorrectTypos", json!(false))?;
            s.set("TabConversion", json!(false))
        })
        .unwrap();
    store
        .record(vec![Change::Language {
            raw: "google".into(),
            english: true,
            explicit: true,
        }])
        .unwrap();
    let mut s = Session::open(store.clone()).unwrap();
    for c in "google".chars() {
        s.key(0, c as i32, 0);
    }
    let view = s.result(false).view.unwrap();
    assert_eq!(view.text, "ごおgぇ");
    assert!(view.hint.contains("Tab → google"));
    // Original UpdateView reserves Suggestion for dictionary misspellings;
    // Manual's language proposal is presented in Hint.
    assert!(view.suggestion.is_none());
    let result = s.key(9, 0, 0);
    assert!(result.consumed);
    assert!(result.commits.is_empty());
    assert_eq!(result.view.unwrap().text, "google");
    assert_eq!(s.commit().commits[0].text, "google");
    for c in "google".chars() {
        s.key(0, c as i32, 0);
    }
    assert_eq!(
        s.display(false),
        "ごおgぇ",
        "language memory does not override Manual"
    );
    s.key(0x75, 0, 0); // Explicit hiragana suppresses the English proposal.
    assert!(!s.result(false).view.unwrap().hint.contains("Tab →"));
}

#[test]
fn space_does_not_accept_an_unselected_english_proposal_in_manual_or_conservative() {
    for (level, raw) in [
        ("Manual", "google"),
        ("Conservative", "no"),
        ("Conservative", "amazon"),
    ] {
        let mut s = session(level, raw, "", "");
        let result = s.key(32, 32, 0);
        assert!(
            result.commits.is_empty(),
            "{level}: {raw} must start conversion instead of committing English"
        );
        assert!(result.view.unwrap().converting);
    }
    let mut s = session("Manual", "google", "", "");
    s.key(9, 0, 0);
    assert_eq!(s.key(32, 32, 0).commits[0].text, "google ");
    let mut capitalized = session("Manual", "Google", "", "");
    assert_eq!(capitalized.key(32, 32, 0).commits[0].text, "Google ");
    // CompositionDetector.IsEnglishAtWordEnd: a trailing consonant at Space
    // identifies a known English word even under Conservative.
    let mut short = session("Conservative", "ok", "", "");
    assert_eq!(short.key(32, 32, 0).commits[0].text, "ok ");
}

#[test]
fn level_changes_preserve_protected_spans_and_manual_commit_does_not_repair_old_text() {
    for level in ["Aggressive", "Conservative", "Manual"] {
        for raw in [
            "https://example.com/a",
            "@name",
            "$skill",
            "/command",
            "C:\\foo\\bar",
        ] {
            assert_eq!(
                session(level, raw, "", "").display(false),
                raw,
                "{level}: {raw}"
            );
        }
    }
    let mut s = session("Manual", "sushi", "", "");
    s.can_delete = true;
    assert_eq!(s.commit().commits[0].text, "すし");
    s.set_context(Some("すし "), Some(""));
    for c in "please".chars() {
        s.key(0, c as i32, 0);
    }
    assert!(s.commit().commits.iter().all(|c| c.delete_before == 0));
}
