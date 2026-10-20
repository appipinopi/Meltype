// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Port of Composition/RomajiTypoCorrector.cs at the migration baseline.
//! Only unreadable syllables are repaired; ambiguous repairs are rejected.
use crate::romaji;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fix {
    pub wrong: String,
    pub right: String,
    pub kana: String,
}
type Cache = HashMap<(String, bool), Option<Fix>>;
pub struct Corrector {
    words: HashSet<String>,
    function: HashSet<String>,
    max_length: usize,
    cache: Mutex<Cache>,
    common_cache: Mutex<HashMap<String, bool>>,
}
impl Corrector {
    pub fn bundled() -> &'static Self {
        static INSTANCE: OnceLock<Corrector> = OnceLock::new();
        INSTANCE.get_or_init(|| Self::new(crate::system_data::text("readings.txt")))
    }
    pub fn new(readings: &str) -> Self {
        let function: HashSet<String> = include_str!("../data/function-words.txt")
            .lines()
            .map(str::to_owned)
            .collect();
        let mut result = Self {
            words: function.clone(),
            function,
            max_length: 0,
            cache: Mutex::new(HashMap::new()),
            common_cache: Mutex::new(HashMap::new()),
        };
        for line in readings.lines().filter(|l| !l.starts_with('#')) {
            let mut fields = line.split('\t');
            let word = fields.next().unwrap_or("").trim();
            if word.is_empty() {
                continue;
            }
            result.words.insert(word.into());
            result.conjugations(word, fields.next().unwrap_or(""));
        }
        result.max_length = result
            .words
            .iter()
            .map(|w| w.chars().count())
            .max()
            .unwrap_or(1);
        result
    }
    fn conjugations(&mut self, word: &str, kind: &str) {
        let Some(last) = word.chars().next_back() else {
            return;
        };
        let stem = &word[..word.len() - last.len_utf8()];
        if stem.is_empty() {
            return;
        }
        let mut add = |end: &str| {
            self.words.insert(format!("{stem}{end}"));
        };
        match kind {
            "v1" if last == 'る' => add(""),
            "i" if last == 'い' => {
                for end in ["く", "かっ", "けれ", "さ", "そう"] {
                    add(end);
                }
                if word == "いい" {
                    for form in ["よく", "よかっ", "よけれ"] {
                        self.words.insert(form.into());
                    }
                }
            }
            "vk" if word.ends_with("くる") => {
                let stem = word.strip_suffix("くる").unwrap();
                for end in ["き", "こ", "く", "くれ"] {
                    self.words.insert(format!("{stem}{end}"));
                }
            }
            "vs" if word.ends_with("する") => {
                let stem = word.strip_suffix("する").unwrap();
                for end in ["し", "さ", "せ", "すれ", "しろ"] {
                    self.words.insert(format!("{stem}{end}"));
                }
            }
            k if k.starts_with("v5") => {
                let row = match last {
                    'う' => "わいえお",
                    'く' => "かきけこ",
                    'ぐ' => "がぎげご",
                    'す' => "さしせそ",
                    'つ' => "たちてと",
                    'ぬ' => "なにねの",
                    'ぶ' => "ばびべぼ",
                    'む' => "まみめも",
                    'る' => "らりれろ",
                    _ => return,
                };
                for c in row.chars() {
                    add(&c.to_string());
                }
                let onbin = match k {
                    "v5k" | "v5g" => Some("い"),
                    "v5k-s" | "v5t" | "v5r" | "v5u" | "v5aru" | "v5r-i" | "v5u-s" => Some("っ"),
                    "v5b" | "v5m" | "v5n" => Some("ん"),
                    _ => None,
                };
                if let Some(end) = onbin {
                    add(end);
                }
                if k == "v5aru" {
                    add("い");
                }
            }
            _ => {}
        }
    }
    pub fn first_unreadable(raw: &str, final_input: bool) -> Option<usize> {
        let analysis = romaji::analyze(raw, true);
        if !analysis.valid {
            Some(analysis.consumed())
        } else if final_input && !analysis.partial.is_empty() && analysis.partial != "n" {
            Some(raw.len() - analysis.partial.len())
        } else {
            None
        }
    }
    pub fn is_common_japanese(&self, raw: &str) -> bool {
        if raw.len() > 128 || !raw.bytes().all(|c| c.is_ascii_alphabetic()) {
            return false;
        }
        if romaji::custom_table().is_some() {
            return self.common_uncached(raw);
        }
        if let Some(result) = self
            .common_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(raw).copied())
        {
            return result;
        }
        let result = self.common_uncached(raw);
        if let Ok(mut cache) = self.common_cache.lock() {
            if cache.len() >= 4096 {
                cache.clear();
            }
            cache.insert(raw.into(), result);
        }
        result
    }
    fn common_uncached(&self, raw: &str) -> bool {
        let analysis = romaji::analyze(&raw.to_ascii_lowercase(), true);
        if !analysis.valid || !analysis.partial.is_empty() && analysis.partial != "n" {
            return false;
        }
        let kana = analysis.kana() + if analysis.partial == "n" { "ん" } else { "" };
        kana.chars().count() >= 2
            && self.segment(&kana, None).0 <= kana.chars().count() as f64 / 2.0
    }
    fn segment(&self, kana: &str, at: Option<usize>) -> (f64, bool) {
        let mut boundaries: Vec<_> = kana.char_indices().map(|(i, _)| i).collect();
        boundaries.push(kana.len());
        let n = boundaries.len() - 1;
        let mut cost = vec![f64::INFINITY; n + 1];
        let mut from = vec![0; n + 1];
        cost[0] = 0.0;
        for i in 1..=n {
            for length in (1..=i.min(self.max_length)).rev() {
                let piece = &kana[boundaries[i - length]..boundaries[i]];
                let step = if self.words.contains(piece) {
                    if self.function.contains(piece) {
                        0.8
                    } else if length >= 2 {
                        1.0
                    } else {
                        1.5
                    }
                } else if length == 1 {
                    if piece == "ー" || piece == "っ" {
                        0.5
                    } else {
                        3.0
                    }
                } else {
                    continue;
                };
                if cost[i - length] + step < cost[i] {
                    cost[i] = cost[i - length] + step;
                    from[i] = i - length;
                }
            }
        }
        let mut known = at.is_none();
        let mut i = n;
        while i > 0 && !known {
            if at.is_some_and(|at| from[i] <= at && at < i) {
                let piece = &kana[boundaries[from[i]]..boundaries[i]];
                known = self.words.contains(piece)
                    && (i - from[i] >= 2 || self.function.contains(piece));
                break;
            }
            i = from[i];
        }
        (cost[n], known)
    }
    pub fn fix(&self, raw: &str, final_input: bool) -> Option<Fix> {
        if raw.len() > 128 || !raw.bytes().all(|c| c.is_ascii_lowercase()) {
            return None;
        }
        if romaji::custom_table().is_some() {
            return self.fix_uncached(raw, final_input);
        }
        let key = (raw.to_owned(), final_input);
        if let Some(value) = self
            .cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&key).cloned())
        {
            return value;
        }
        let result = self.fix_uncached(raw, final_input);
        if let Ok(mut cache) = self.cache.lock() {
            if cache.len() >= 1024 {
                cache.clear();
            }
            cache.insert(key, result.clone());
        }
        result
    }
    fn fix_uncached(&self, raw: &str, final_input: bool) -> Option<Fix> {
        let first = Self::first_unreadable(raw, final_input)?;
        if first == 0 || !final_input && raw.len() - first - 1 < 2 {
            return None;
        }
        let bytes = raw.as_bytes();
        if first == raw.len() - 1 && romaji::analyze(raw, true).valid {
            let last = bytes[first];
            let previous = bytes[first - 1];
            if last != previous && !neighbors(last).contains(previous as char) {
                return None;
            }
            let right = &raw[..first];
            let kana = romaji::hiragana(right, true);
            return self
                .segment(&kana, Some(kana.chars().count().saturating_sub(1)))
                .1
                .then(|| Fix {
                    wrong: raw.into(),
                    right: right.into(),
                    kana,
                });
        }
        let mut candidates: BTreeMap<String, (f64, usize)> = BTreeMap::new();
        let mut consider = |candidate: String, edit, prior| {
            if Self::first_unreadable(&candidate, final_input).is_some() {
                return;
            }
            let entry = candidates.entry(candidate).or_insert((f64::INFINITY, edit));
            if prior < entry.0 {
                *entry = (prior, edit);
            }
        };
        for k in first.saturating_sub(2).max(1)..=(first + 2).min(raw.len()) {
            if k < raw.len() {
                let near = neighbors(bytes[k]);
                for c in near.chars() {
                    consider(format!("{}{c}{}", &raw[..k], &raw[k + 1..]), k, 0.3);
                }
                if bytes[k] == bytes[k - 1]
                    || near.contains(bytes[k - 1] as char)
                    || bytes.get(k + 1).is_some_and(|c| near.contains(*c as char))
                {
                    consider(format!("{}{}", &raw[..k], &raw[k + 1..]), k, 0.5);
                }
                if k + 1 < raw.len() && bytes[k] != bytes[k + 1] {
                    consider(
                        format!(
                            "{}{}{}{}",
                            &raw[..k],
                            bytes[k + 1] as char,
                            bytes[k] as char,
                            &raw[k + 2..]
                        ),
                        k,
                        0.3,
                    );
                }
            }
            for c in b'a'..=b'z' {
                consider(
                    format!("{}{}{}", &raw[..k], c as char, &raw[k..]),
                    k,
                    if b"aiueo".contains(&c) { 0.3 } else { 0.8 },
                );
            }
        }
        let mut ranked = Vec::new();
        for (candidate, (prior, edit)) in candidates {
            let analysis = romaji::analyze(&candidate, true);
            let kana = analysis.kana()
                + if final_input && analysis.partial == "n" {
                    "ん"
                } else {
                    ""
                };
            let mut position = 0;
            let mut kana_position = 0;
            for token in &analysis.tokens {
                if edit < position + token.spelling.len() {
                    break;
                }
                position += token.spelling.len();
                kana_position += token.kana.chars().count();
            }
            let at = kana_position.min(kana.chars().count().saturating_sub(1));
            let (cost, known) = self.segment(&kana, Some(at));
            if known {
                ranked.push((cost + prior, candidate, kana));
            }
        }
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let best = ranked.first()?;
        if ranked
            .get(1)
            .is_some_and(|second| second.0 - best.0 < 0.2 && second.2 != best.2)
        {
            return None;
        }
        if best.0 >= self.segment(&romaji::hiragana(raw, final_input), None).0 {
            return None;
        }
        Some(Fix {
            wrong: raw.into(),
            right: best.1.clone(),
            kana: best.2.clone(),
        })
    }
}
fn neighbors(c: u8) -> &'static str {
    match c {
        b'q' => "wa",
        b'w' => "qeas",
        b'e' => "wrsd",
        b'r' => "etdf",
        b't' => "ryfg",
        b'y' => "tugh",
        b'u' => "yihj",
        b'i' => "uojk",
        b'o' => "ipkl",
        b'p' => "ol",
        b'a' => "qwsz",
        b's' => "weadzx",
        b'd' => "erfsxc",
        b'f' => "rtdgcv",
        b'g' => "tyfhvb",
        b'h' => "yugjbn",
        b'j' => "uihknm",
        b'k' => "iojlm",
        b'l' => "opk",
        b'z' => "asx",
        b'x' => "zsdc",
        b'c' => "xdfv",
        b'v' => "cfgb",
        b'b' => "vghn",
        b'n' => "bhjm",
        b'm' => "njk",
        _ => "",
    }
}
