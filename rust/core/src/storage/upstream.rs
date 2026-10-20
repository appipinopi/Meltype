// SPDX-License-Identifier: GPL-3.0-or-later
//! Projections for the upstream application's seven separate data files.
use super::*;

pub(super) const FILES: &[&str] = &[
    "model.json",
    "languages.json",
    "conversions.json",
    "translations.json",
    "snippets.json",
    "userdict.txt",
    "phrases.txt",
];

fn timestamp(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl Store {
    pub(super) fn upstream_files(&self, state: &State) -> io::Result<BTreeMap<String, Vec<u8>>> {
        let mut files = BTreeMap::new();
        let languages: BTreeMap<_, _> = state.language_counts.iter().map(|(word, e)| {
            (word, json!({"English":e.english,"Count":e.count,"Explicit":e.explicit,"Used":timestamp(e.used)}))
        }).collect();
        let mut languages = serde_json::to_value(languages)?;
        // Preserve sub-second timestamps from files imported from C# when
        // the corresponding entry is unchanged at Rust's timestamp precision.
        if let Some(old) = self.upstream_json("languages.json")? {
            for (word, entry) in languages.as_object_mut().unwrap() {
                let source = &old[word];
                if source["English"] == entry["English"]
                    && source["Count"] == entry["Count"]
                    && source["Explicit"] == entry["Explicit"]
                    && parse_time(&source["Used"]) == parse_time(&entry["Used"])
                    && source["Used"].is_string()
                {
                    entry["Used"] = source["Used"].clone();
                }
            }
        }
        let old = self
            .upstream_json("conversions.json")?
            .unwrap_or(Value::Null);
        let conversions: BTreeMap<_, _> = state
            .preferences
            .selected
            .iter()
            .map(|(reading, text)| {
                let used = state
                    .used
                    .get(&format!("conversion:{reading}"))
                    .copied()
                    .unwrap_or(0);
                let time = if old[reading]["Text"] == *text
                    && old[reading]["Used"].is_string()
                    && parse_time(&old[reading]["Used"]) == used
                {
                    old[reading]["Used"].clone()
                } else {
                    json!(timestamp(used))
                };
                (reading, json!({"Text":text,"Used":time}))
            })
            .collect();
        for (name, value) in [
            ("model.json", serde_json::to_value(&state.prefix_model)?),
            ("languages.json", languages),
            ("conversions.json", serde_json::to_value(conversions)?),
            (
                "translations.json",
                serde_json::to_value(&state.translations)?,
            ),
            ("snippets.json", serde_json::to_value(&state.snippets)?),
        ] {
            files.insert(name.into(), serde_json::to_vec_pretty(&value)?);
        }
        let mut dictionary = String::new();
        for (reading, words) in &state.preferences.user_dictionary {
            // Upstream loads text lines in registration order, then reverses
            // each reading so the most recently added candidate comes first.
            for word in words.iter().rev() {
                if reading.encode_utf16().count() >= 2
                    && !word.is_empty()
                    && !reading.contains(['\t', '\r', '\n'])
                    && !word.contains(['\t', '\r', '\n'])
                {
                    dictionary.push_str(&format!("{reading}\t{word}\n"));
                }
            }
        }
        files.insert("userdict.txt".into(), dictionary.into_bytes());
        let mut phrases = String::new();
        let mut old_phrases = BTreeMap::new();
        match fs::read(self.directory.join("phrases.txt")) {
            Ok(bytes) => {
                let (text, _) = crate::user_dictionary::decode(&bytes).unwrap_or_default();
                for line in text.lines() {
                    let p: Vec<_> = line.split('\t').collect();
                    if p.len() >= 4 {
                        if let (Ok(count), Ok(ticks)) = (p[2].parse::<i32>(), p[3].parse::<i64>()) {
                            old_phrases.insert(
                                (p[0].to_owned(), p[1].to_owned()),
                                (count.max(1) as u32, ticks),
                            );
                        }
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        for phrase in &state.phrases {
            let original = old_phrases
                .get(&(phrase.reading.clone(), phrase.text.clone()))
                .filter(|(count, ticks)| {
                    *count == phrase.count
                        && ticks.div_euclid(10_000_000) - 62_135_596_800 == phrase.used
                })
                .map(|(_, ticks)| *ticks);
            let ticks = original
                .or_else(|| {
                    phrase
                        .used
                        .checked_add(62_135_596_800)
                        .and_then(|n| n.checked_mul(10_000_000))
                })
                .ok_or_else(|| io::Error::other("phrase timestamp outside upstream range"))?;
            phrases.push_str(&format!(
                "{}\t{}\t{}\t{ticks}\n",
                phrase.reading, phrase.text, phrase.count
            ));
        }
        files.insert("phrases.txt".into(), phrases.into_bytes());
        Ok(files)
    }
    fn upstream_json(&self, name: &str) -> io::Result<Option<Value>> {
        match fs::read(self.directory.join(name)) {
            Ok(bytes) => Ok(crate::user_dictionary::decode(&bytes)
                .ok()
                .and_then(|(text, _)| serde_json::from_str(&text).ok())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}
