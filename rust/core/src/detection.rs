// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! ScoreEngine, dictionary, English, kana and typo contributions from upstream.
use crate::{romaji, settings::Settings};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Japanese,
    English,
    Unknown,
    Undecided,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contribution {
    pub source: String,
    pub japanese: i32,
    pub english: i32,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Result {
    pub verdict: Verdict,
    pub text: String,
    pub japanese: i32,
    pub english: i32,
    pub contributions: Vec<Contribution>,
}
struct Lexicon {
    japanese: HashSet<String>,
    prefixes: HashSet<String>,
    english: HashSet<String>,
    english_prefixes: HashSet<String>,
    kana: HashSet<String>,
    kana_prefixes: HashSet<String>,
}
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .flat_map(|l| l.split('#').next().unwrap_or("").split_whitespace())
}
fn prefixes(words: &HashSet<String>) -> HashSet<String> {
    words
        .iter()
        .flat_map(|word| {
            word.char_indices()
                .map(|(i, c)| word[..i + c.len_utf8()].to_owned())
        })
        .collect()
}
fn lexicon() -> &'static Lexicon {
    static LEXICON: OnceLock<Lexicon> = OnceLock::new();
    LEXICON.get_or_init(|| {
        let _builtins = romaji::Scope::new(None);
        let mut japanese = HashSet::new();
        let mut kana = HashSet::new();
        for word in words(include_str!("../../../dictionaries/japanese.txt")) {
            japanese.extend(romaji::spelling_variants(word, 64));
            let mut analysis = romaji::analyze(word, false);
            if analysis.valid && analysis.partial == "n" {
                analysis = romaji::analyze(&format!("{word}n"), false);
            }
            if analysis.valid && analysis.partial.is_empty() {
                kana.insert(analysis.kana());
            }
        }
        let english: HashSet<String> = words(include_str!("../../../dictionaries/english.txt"))
            .chain(words(include_str!("../../../dictionaries/propernouns.txt")))
            .map(str::to_ascii_lowercase)
            .collect();
        Lexicon {
            prefixes: prefixes(&japanese),
            english_prefixes: prefixes(&english),
            kana_prefixes: prefixes(&kana),
            japanese,
            english,
            kana,
        }
    })
}
const PARTICLES: &[&str] = &[
    "no", "ga", "wo", "ni", "de", "to", "ha", "mo", "wa", "he", "ya",
];
/// TypoDetector.Evaluate, also used by CompositionDetector's unknown-word rule.
pub(crate) fn japanese_typo(text: &str) -> bool {
    if text.len() < 5 {
        return false;
    }
    let lex = lexicon();
    if lex.prefixes.contains(text) {
        return false;
    }
    lex.japanese
        .iter()
        .filter(|w| w.as_bytes().first() == text.as_bytes().first())
        .any(|word| {
            (text.len() - 1..=text.len() + 1).any(|n| {
                word.get(..n)
                    .is_some_and(|prefix| crate::linguistics::distance(text, prefix, 1) <= 1)
            })
        })
}
pub fn evaluate(raw: &str, keys: &[i32], final_input: bool, settings: &Settings) -> Result {
    let lower = raw.to_ascii_lowercase();
    let mut bytes = lower.as_bytes().to_vec();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b'c' && b"auo".contains(&bytes[i + 1]) {
            bytes[i] = b'k';
        }
    }
    let text = String::from_utf8(bytes).unwrap_or(lower.clone());
    let mut contributions = Vec::new();
    let result = |verdict, contributions: Vec<Contribution>| Result {
        verdict,
        text: text.clone(),
        japanese: contributions.iter().map(|c| c.japanese).sum(),
        english: contributions.iter().map(|c| c.english).sum(),
        contributions,
    };
    if text.is_empty() {
        return result(
            if final_input {
                Verdict::Unknown
            } else {
                Verdict::Undecided
            },
            contributions,
        );
    }
    let use_romaji = settings.text("InputStyle") != "Kana";
    let use_kana = settings.text("InputStyle") != "Romaji";
    if use_romaji && !use_kana && ["l", "q", "v", "x"].contains(&text.as_str()) {
        return result(Verdict::English, contributions);
    }
    let lex = lexicon();
    let mut add = |source: &str, japanese, english, reason: &str| {
        contributions.push(Contribution {
            source: source.into(),
            japanese,
            english,
            reason: reason.into(),
        });
    };
    let mut romaji_valid = false;
    let mut jp_prefix = false;
    if use_romaji {
        let analysis = romaji::analyze(&text, false);
        romaji_valid = analysis.valid;
        if !analysis.valid {
            add(
                "Romaji",
                0,
                if use_kana { 0 } else { 5 },
                "ローマ字として成立しない",
            );
        } else {
            if analysis.strong_youon > 0 {
                add("Romaji", 3, 0, "日本語特有の拗音");
            }
            if analysis.tsu > 0 {
                add("Romaji", 3, 0, "tsu");
            }
            if analysis.long_vowels > 0 {
                add("Romaji", 1, 0, "長音");
            }
            if analysis.sokuon > 0 {
                add("Romaji", 1, 0, "促音");
            }
            jp_prefix = lex.prefixes.contains(&text);
            if text.len() >= 3 {
                if lex.japanese.contains(&text) {
                    add("Dictionary", 5, 0, "日本語辞書の語と一致");
                } else if jp_prefix {
                    add(
                        "Dictionary",
                        if text.len() >= 4 { 4 } else { 3 },
                        0,
                        "日本語辞書の語の先頭と一致",
                    );
                } else if PARTICLES.iter().any(|p| {
                    text.strip_prefix(p)
                        .is_some_and(|rest| rest.len() >= 4 && lex.prefixes.contains(rest))
                }) {
                    add("Dictionary", 4, 0, "助詞と日本語辞書の語");
                }
            }
            if settings.boolean("TypoEnabled")
                && text.len() >= 5
                && !jp_prefix
                && lex
                    .japanese
                    .iter()
                    .filter(|w| w.as_bytes().first() == text.as_bytes().first())
                    .any(|word| {
                        (text.len() - 1..=text.len() + 1).any(|n| {
                            word.get(..n).is_some_and(|prefix| {
                                crate::linguistics::distance(&text, prefix, 1) <= 1
                            })
                        })
                    })
            {
                add("Typo", 3, 0, "日本語辞書の綴りと編集距離1以内");
            }
            if text.len() >= 5
                && analysis.partial.is_empty()
                && !(lower.len() >= 2 && lex.english_prefixes.contains(&lower))
            {
                add(
                    "Romaji",
                    if text.len() >= 6 { 4 } else { 3 },
                    0,
                    "すべてローマ字で読め、英語の接頭辞に当たらない",
                );
            }
        }
    }
    let mut kana_plausible = false;
    if use_kana {
        let mut kana = String::new();
        let mut valid = true;
        for key in keys {
            if let Some(c) = crate::kana::key(*key, false) {
                if "゛゜".contains(c) {
                    if let Some(combined) = kana
                        .chars()
                        .next_back()
                        .and_then(|previous| crate::kana::combine(previous, c))
                    {
                        kana.pop();
                        kana.push(combined);
                    } else {
                        valid = false;
                        break;
                    }
                } else {
                    kana.push(c);
                }
            } else {
                valid = false;
                break;
            }
        }
        if valid && !kana.starts_with(['ん', 'ー']) {
            let length = kana.chars().count();
            if length >= 2 && lex.kana.contains(&kana) {
                add("Kana", 5, 0, "かな入力が日本語辞書と一致");
                kana_plausible = true;
            } else if length >= 2 && lex.kana_prefixes.contains(&kana) {
                add(
                    "Kana",
                    if length >= 3 { 4 } else { 2 },
                    0,
                    "かな入力が日本語辞書の先頭と一致",
                );
                kana_plausible = true;
            } else {
                kana_plausible = length < 3;
            }
        }
    }
    if lower.len() >= 2 {
        if lex.english.contains(&lower) {
            add("English", 0, 4, "英語辞書の語と一致");
        } else if lex.english_prefixes.contains(&lower) {
            add("English", 0, 3, "英語辞書の語の先頭と一致");
        }
    }
    let japanese: i32 = contributions.iter().map(|c| c.japanese).sum();
    let english: i32 = contributions.iter().map(|c| c.english).sum();
    let threshold = (settings.integer("JapaneseThreshold") as i32
        + match settings.text("DetectionLevel") {
            "Aggressive" => -1,
            "Conservative" => 3,
            _ => 0,
        })
    .max(2);
    let particle_then_more = romaji_valid
        && !final_input
        && PARTICLES
            .iter()
            .find(|p| text.starts_with(**p))
            .is_some_and(|p| (1..4).contains(&(text.len() - p.len())));
    let verdict = if !romaji_valid && !kana_plausible {
        Verdict::English
    } else if japanese >= threshold && japanese - english >= threshold {
        Verdict::Japanese
    } else if english >= 3
        && japanese < threshold
        && !jp_prefix
        && !particle_then_more
        && !(use_kana && kana_plausible)
        && text.len() >= 2
    {
        Verdict::English
    } else if final_input || text.len() >= settings.integer("MaxPendingKeys") as usize {
        Verdict::Unknown
    } else {
        Verdict::Undecided
    };
    result(verdict, contributions)
}
