// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
use crate::{romaji, storage::State};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::OnceLock,
};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

pub struct Linguistics {
    meanings: HashMap<String, Vec<(String, String)>>,
    stems: HashMap<String, String>,
    translations: HashMap<String, Vec<(String, Vec<String>)>>,
    contexts: HashMap<String, Vec<(String, Vec<String>)>>,
    misspellings: Vec<(String, String)>,
    correct: HashSet<String>,
    shapes: HashMap<String, Vec<String>>,
    ascii_misspellings: bool,
    pub readings: BTreeMap<String, Vec<String>>,
}
pub fn kanji(c: char) -> bool {
    ('㐀'..='䶿').contains(&c) || ('一'..='鿿').contains(&c) || c == '々'
}
pub fn kana(c: char) -> bool {
    ('ぁ'..='ゖ').contains(&c) || ('ァ'..='ヶ').contains(&c) || c == 'ー'
}
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
}
fn ends(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.char_indices().map(|(i, c)| i + c.len_utf8()).rev()
}
impl Linguistics {
    pub fn bundled() -> &'static Self {
        static DATA: OnceLock<Linguistics> = OnceLock::new();
        DATA.get_or_init(|| {
            let mut d = Self {
                meanings: HashMap::new(),
                stems: HashMap::new(),
                translations: HashMap::new(),
                contexts: HashMap::new(),
                misspellings: vec![],
                correct: HashSet::new(),
                shapes: HashMap::new(),
                ascii_misspellings: false,
                readings: BTreeMap::new(),
            };
            for line in lines(include_str!("../../../dictionaries/meanings.txt")) {
                let fields: Vec<_> = line.split('\t').collect();
                if fields.len() < 3 {
                    continue;
                }
                let (word, reading, gloss) = (fields[0], fields[1], fields[2]);
                d.meanings
                    .entry(word.into())
                    .or_default()
                    .push((reading.into(), gloss.into()));
                if let Some((i, last)) = word.char_indices().next_back() {
                    if i > 0
                        && "うくぐすつぬぶむるい".contains(last)
                        && word[..i].chars().any(kanji)
                    {
                        d.stems.entry(word[..i].into()).or_insert(word.into());
                    }
                }
                if reading.chars().count() >= 2
                    && reading.chars().all(kana)
                    && word.chars().any(kanji)
                {
                    let entry = d
                        .readings
                        .entry(crate::user_dictionary::hiragana(reading))
                        .or_default();
                    if !entry.iter().any(|w| w == word) {
                        entry.push(word.into());
                    }
                }
            }
            for line in lines(include_str!("../../../dictionaries/translations.txt")) {
                let p: Vec<_> = line.split('\t').collect();
                if p.len() >= 3 {
                    d.translations.entry(p[0].into()).or_default().push((
                        p[1].into(),
                        p[2].split(',')
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned)
                            .collect(),
                    ));
                }
            }
            for line in lines(include_str!("../../../dictionaries/contexts.txt")) {
                let line = line.split('#').next().unwrap_or("");
                if let Some((head, cues)) = line.split_once(':') {
                    let head: Vec<_> = head.split_whitespace().collect();
                    if head.len() == 2 {
                        d.contexts.entry(head[0].into()).or_default().push((
                            head[1].into(),
                            cues.split_whitespace().map(str::to_owned).collect(),
                        ));
                    }
                }
            }
            for line in lines(include_str!("../../../dictionaries/misspellings.txt")) {
                let p: Vec<_> = line.split('\t').collect();
                let right = romaji::katakana(p[p.len() - 1]);
                if p.len() > 1 {
                    d.misspellings.push((romaji::katakana(p[0]), right.clone()));
                }
                d.correct.insert(right.clone());
                if right.chars().count() >= 5 {
                    d.shapes.entry(shape(&right)).or_default().push(right);
                }
            }
            // ASCII readings cannot match non-ASCII spellings or normalized
            // shapes. Derive this from the loaded dictionary, including empty
            // shapes, so adding an ASCII entry keeps the normal lookup path.
            d.ascii_misspellings = d.misspellings.iter().any(|(wrong, _)| wrong.is_ascii())
                || d.shapes.keys().any(|key| key.is_ascii());
            d.misspellings
                .sort_by_key(|(wrong, _)| std::cmp::Reverse(wrong.len()));
            d
        })
    }
    pub fn context(&self, reading: &str, surrounding: &str) -> Option<String> {
        for end in ends(reading) {
            if let Some(rules) = self.contexts.get(&reading[..end]) {
                return rules
                    .iter()
                    .map(|(candidate, cues)| {
                        (
                            candidate,
                            cues.iter()
                                .filter(|s| surrounding.contains(s.as_str()))
                                .count(),
                        )
                    })
                    .filter(|(_, hits)| *hits > 0)
                    .max_by_key(|(_, hits)| *hits)
                    .map(|(candidate, _)| candidate.clone() + &reading[end..]);
            }
        }
        None
    }
    pub fn meaning(&self, text: &str, reading: &str) -> Option<String> {
        if !text.chars().any(|c| kanji(c) || ('ァ'..='ヶ').contains(&c)) {
            return None;
        }
        for end in ends(text) {
            let word = &text[..end];
            let senses = self
                .meanings
                .get(word)
                .or_else(|| self.stems.get(word).and_then(|w| self.meanings.get(w)));
            if let Some(senses) = senses {
                if text[end..]
                    .chars()
                    .any(|c| kanji(c) || ('ァ'..='ヶ').contains(&c))
                {
                    return None;
                }
                let mut selected: Vec<_> = senses
                    .iter()
                    .filter(|(r, _)| !r.is_empty() && reading.starts_with(r))
                    .collect();
                selected.extend(senses.iter().filter(|(r, _)| r.is_empty()));
                if selected.is_empty() {
                    selected = senses.iter().collect();
                }
                let selected: Vec<_> = selected
                    .into_iter()
                    .take(2)
                    .map(|(_, g)| g.as_str())
                    .collect();
                return Some(if selected.len() == 1 {
                    selected[0].into()
                } else {
                    selected
                        .iter()
                        .enumerate()
                        .map(|(i, g)| {
                            format!("{} {g}", char::from_u32('①' as u32 + i as u32).unwrap())
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
        }
        None
    }
    pub fn translations(&self, text: &str, reading: &str) -> Vec<String> {
        for key in [text, reading] {
            for end in ends(key) {
                if let Some(entries) = self.translations.get(&key[..end]) {
                    let suffix = &key[end..];
                    if ![
                        "", "な", "に", "だ", "です", "の", "を", "が", "は", "も", "と", "で",
                        "へ", "や", "から", "まで", "より", "って", "とか", "さ",
                    ]
                    .contains(&suffix)
                    {
                        break;
                    }
                    let order = match suffix {
                        "な" => ["na", "i", "n", "adv", "v"],
                        "に" => ["adv", "na", "n", "i", "v"],
                        _ => ["n", "na", "i", "v", "adv"],
                    };
                    let mut words = vec![];
                    for kind in order {
                        for (entry_kind, translations) in entries {
                            if entry_kind == kind {
                                let tail = if suffix == "な"
                                    || suffix == "に" && ["adv", "na"].contains(&kind)
                                {
                                    ""
                                } else {
                                    suffix
                                };
                                for word in translations {
                                    let word = format!("{word}{tail}");
                                    if words.len() < 5 && !words.contains(&word) {
                                        words.push(word);
                                    }
                                }
                            }
                        }
                    }
                    return words;
                }
            }
        }
        vec![]
    }
    pub fn misspelling(&self, reading: &str) -> Option<String> {
        if reading.is_ascii() && !self.ascii_misspellings {
            return None;
        }
        let text = romaji::katakana(reading);
        let inside_correct = |start: usize, len: usize| {
            self.correct.iter().any(|word| {
                word.len() > len
                    && text
                        .match_indices(word)
                        .any(|(i, _)| i <= start && i + word.len() >= start + len)
            })
        };
        for (wrong, right) in &self.misspellings {
            if let Some(i) = text.find(wrong) {
                if !inside_correct(i, wrong.len()) {
                    return Some(text[..i].to_owned() + right + &text[i + wrong.len()..]);
                }
            }
        }
        let bounds: Vec<_> = text
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(text.len()))
            .collect();
        for start in 0..bounds.len() - 1 {
            for end in (start + 4..=usize::min(start + 20, bounds.len() - 1)).rev() {
                let part = &text[bounds[start]..bounds[end]];
                if self.correct.contains(part) {
                    break;
                }
                if let Some(right) = self.shapes.get(&shape(part)).and_then(|s| s.first()) {
                    if !inside_correct(bounds[start], part.len()) {
                        return Some(
                            text[..bounds[start]].to_owned() + right + &text[bounds[end]..],
                        );
                    }
                }
            }
        }
        None
    }
    pub fn predict(&self, reading: &str, state: &State) -> Vec<String> {
        if reading.chars().count() < 2 {
            return vec![];
        }
        let mut phrases: Vec<_> = state
            .phrases
            .iter()
            .filter(|p| p.reading.starts_with(reading) && p.reading != reading)
            .collect();
        phrases.sort_by_key(|p| (std::cmp::Reverse(p.count), std::cmp::Reverse(p.used)));
        let mut result = vec![];
        for word in phrases
            .into_iter()
            .map(|p| p.text.clone())
            .chain(
                state
                    .preferences
                    .user_dictionary
                    .iter()
                    .filter(|(r, _)| r.starts_with(reading) && r.as_str() != reading)
                    .flat_map(|(_, v)| v.clone()),
            )
            .chain(
                state
                    .preferences
                    .selected
                    .iter()
                    .filter(|(r, _)| r.starts_with(reading) && r.as_str() != reading)
                    .map(|(_, v)| v.clone()),
            )
        {
            if !result.contains(&word) {
                result.push(word);
                if result.len() == 5 {
                    break;
                }
            }
        }
        result
    }
    pub fn words(&self, reading: &str) -> Vec<String> {
        self.readings.get(reading).cloned().unwrap_or_default()
    }
}
fn shape(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c) && !"ッー".contains(*c))
        .map(|c| match c {
            'ァ' => 'ア',
            'ィ' => 'イ',
            'ゥ' => 'ウ',
            'ェ' => 'エ',
            'ォ' => 'オ',
            'ャ' => 'ヤ',
            'ュ' => 'ユ',
            'ョ' => 'ヨ',
            'ヮ' => 'ワ',
            _ => c,
        })
        .collect()
}
pub fn distance(a: &str, b: &str, max: usize) -> usize {
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return max + 1;
    }
    let mut prev: Vec<_> = (0..=b.len()).collect();
    for (i, a) in a.iter().enumerate() {
        let mut row = vec![i + 1; b.len() + 1];
        for (j, b) in b.iter().enumerate() {
            row[j + 1] = (prev[j + 1] + 1)
                .min(row[j] + 1)
                .min(prev[j] + usize::from(a != b));
        }
        if *row.iter().min().unwrap() > max {
            return max + 1;
        }
        prev = row;
    }
    prev[b.len()]
}
