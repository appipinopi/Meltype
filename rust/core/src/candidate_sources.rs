// SPDX-License-Identifier: GPL-3.0-or-later
//! Ordered auxiliary candidates and context rules, owned by a profile.
use std::{
    collections::BTreeMap,
    io,
    path::Path,
    sync::{Arc, OnceLock},
};
#[derive(Clone, Default)]
pub(crate) struct Sources {
    words: BTreeMap<String, Vec<String>>,
    emoji: BTreeMap<String, Vec<String>>,
    contexts: BTreeMap<String, Vec<(String, Vec<String>)>>,
    imported: BTreeMap<String, Vec<String>>,
    phrases: BTreeMap<String, Vec<String>>,
}
fn add(
    map: &mut BTreeMap<String, Vec<String>>,
    reading: &str,
    values: impl Iterator<Item = String>,
) {
    let words = map.entry(reading.into()).or_default();
    for word in values {
        if !word.is_empty() && !words.contains(&word) {
            words.push(word);
        }
    }
}
fn lookup(map: &BTreeMap<String, Vec<String>>, reading: &str) -> Vec<String> {
    for end in reading.char_indices().map(|(i, c)| i + c.len_utf8()).rev() {
        if reading[..end].encode_utf16().count() == 1 && end < reading.len() {
            break;
        }
        let suffix = &reading[end..];
        if ![
            "", "を", "が", "は", "に", "で", "と", "も", "へ", "の", "や", "な", "だ", "です",
            "から", "まで", "より", "って", "とか", "さ", "ね", "よ",
        ]
        .contains(&suffix)
        {
            continue;
        }
        if let Some(words) = map.get(&reading[..end]) {
            return words.iter().map(|w| format!("{w}{suffix}")).collect();
        }
    }
    vec![]
}
impl Sources {
    pub fn bundled() -> Arc<Self> {
        static SOURCES: OnceLock<Arc<Sources>> = OnceLock::new();
        SOURCES
            .get_or_init(|| {
                let mut source = Self::default();
                for text in [
                    include_str!("../../../dictionaries/candidates.txt"),
                    include_str!("../../../dictionaries/loanwords.txt"),
                    include_str!("../../../dictionaries/brands.txt"),
                ] {
                    source.add_words(text);
                }
                for text in [
                    include_str!("../../../dictionaries/emoji.txt"),
                    include_str!("../../../dictionaries/emoji-cldr.txt"),
                ] {
                    source.add_emoji(text);
                }
                source.add_contexts(include_str!("../../../dictionaries/contexts.txt"));
                for line in include_str!("../../../dictionaries/phrases.txt")
                    .lines()
                    .filter(|line| !line.starts_with('#'))
                {
                    let mut parts = line.split('\t');
                    if let (Some(reading), Some(word)) = (parts.next(), parts.next()) {
                        add(
                            &mut source.phrases,
                            reading,
                            std::iter::once(word.to_owned()),
                        );
                    }
                }
                Arc::new(source)
            })
            .clone()
    }
    pub fn load(directory: &Path) -> io::Result<Arc<Self>> {
        let files = ["candidates.txt", "emoji.txt", "contexts.txt"];
        let mut plists: Vec<_> = std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "plist"))
            .collect();
        plists.sort();
        if plists.is_empty() && !files.iter().any(|name| directory.join(name).exists()) {
            return Ok(Self::bundled());
        }
        let mut source = (*Self::bundled()).clone();
        for name in files {
            if let Ok(bytes) = std::fs::read(directory.join(name)) {
                let Ok((text, _)) = crate::user_dictionary::decode(&bytes) else {
                    continue;
                };
                match name {
                    "candidates.txt" => source.add_words(&text),
                    "emoji.txt" => source.add_emoji(&text),
                    _ => source.add_contexts(&text),
                }
            }
        }
        for path in plists {
            if let Some(import) = std::fs::read(path)
                .ok()
                .and_then(|bytes| crate::user_dictionary::parse(&bytes).ok())
            {
                for (reading, word) in import.words {
                    add(&mut source.imported, &reading, std::iter::once(word));
                }
            }
        }
        Ok(Arc::new(source))
    }
    pub fn registered(&self, reading: &str) -> Vec<String> {
        self.imported
            .get(reading)
            .into_iter()
            .flatten()
            .chain(self.phrases.get(reading).into_iter().flatten())
            .cloned()
            .collect()
    }
    pub fn registrations(&self) -> BTreeMap<String, (String, bool)> {
        let mut words: BTreeMap<_, _> = self
            .phrases
            .iter()
            .filter_map(|(reading, words)| {
                words
                    .first()
                    .map(|word| (reading.clone(), (word.clone(), false)))
            })
            .collect();
        for (reading, entries) in &self.imported {
            if let Some(word) = entries.first() {
                words.insert(reading.clone(), (word.clone(), true));
            }
        }
        words
    }
    fn add_words(&mut self, text: &str) {
        for line in text.lines() {
            let words: Vec<_> = line
                .split('#')
                .next()
                .unwrap_or("")
                .split([' ', '\t', '\r', '　'])
                .filter(|w| !w.is_empty())
                .collect();
            if words.len() > 1 {
                add(
                    &mut self.words,
                    words[0],
                    words[1..].iter().map(|w| w.to_string()),
                );
            }
        }
    }
    fn add_emoji(&mut self, text: &str) {
        for line in text
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
        {
            let words: Vec<_> = line
                .split('\t')
                .map(str::trim)
                .filter(|w| !w.is_empty())
                .collect();
            if words.len() > 1 {
                add(
                    &mut self.emoji,
                    words[0],
                    words[1..].iter().map(|w| w.to_string()),
                );
            }
        }
    }
    fn add_contexts(&mut self, text: &str) {
        for line in text.lines() {
            if let Some((head, cues)) = line.split('#').next().unwrap_or("").split_once(':') {
                let head: Vec<_> = head.split_whitespace().collect();
                let cues: Vec<_> = cues.split_whitespace().map(str::to_owned).collect();
                if head.len() == 2 && !cues.is_empty() {
                    self.contexts
                        .entry(head[0].into())
                        .or_default()
                        .push((head[1].into(), cues));
                }
            }
        }
    }
    pub fn context(&self, reading: &str, surrounding: &str) -> Option<String> {
        for end in reading.char_indices().map(|(i, c)| i + c.len_utf8()).rev() {
            if let Some(rules) = self.contexts.get(&reading[..end]) {
                let mut best = None;
                let mut score = 0;
                for (candidate, cues) in rules {
                    let hits = cues
                        .iter()
                        .filter(|cue| surrounding.contains(cue.as_str()))
                        .count();
                    if hits > score {
                        score = hits;
                        best = Some(format!("{candidate}{}", &reading[end..]));
                    }
                }
                return best;
            }
        }
        None
    }
    pub fn candidates(&self, reading: &str) -> Vec<String> {
        let mut words = lookup(&self.words, reading);
        let katakana = crate::romaji::katakana(reading);
        words.extend([
            reading.into(),
            katakana.clone(),
            crate::kana::halfwidth(&katakana),
        ]);
        words.extend(crate::time_candidates::candidates(
            reading,
            chrono::Local::now().naive_local(),
        ));
        words.extend(self.emoji(reading));
        words
    }
    pub fn emoji(&self, reading: &str) -> Vec<String> {
        lookup(&self.emoji, reading).into_iter().rev().collect()
    }
}
