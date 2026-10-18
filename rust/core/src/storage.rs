// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Durable user data. Every mutation re-reads under an OS file lock, preventing
//! lost updates between IME sessions, settings windows and other processes.
use crate::{session::Preferences, settings::Settings};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("file has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Store {
    pub directory: PathBuf,
}
struct Lock(File);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub prefix_model: crate::prefix_learning::Model,
    pub preferences: Preferences,
    pub language_counts: BTreeMap<String, LanguageEntry>,
    pub translations: BTreeMap<String, BTreeMap<String, u32>>,
    pub phrases: Vec<Phrase>,
    pub snippets: BTreeMap<String, String>,
    pub used: BTreeMap<String, i64>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LanguageEntry {
    pub english: bool,
    pub count: u32,
    pub explicit: bool,
    pub used: i64,
}
impl LanguageEntry {
    pub fn active(&self, word: &str) -> bool {
        if !self.english || self.count.max(1) >= 2 {
            return true;
        }
        let analysis = crate::romaji::analyze(word, true);
        !(crate::typo::Corrector::bundled().is_common_japanese(word)
            || !self.explicit
                && word.len() <= 2
                && analysis.valid
                && (analysis.partial.is_empty() || analysis.partial == "n"))
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Phrase {
    pub reading: String,
    pub text: String,
    pub count: u32,
    pub used: i64,
}

#[derive(Clone)]
pub enum Change {
    Language {
        raw: String,
        english: bool,
        explicit: bool,
    },
    Conversion {
        reading: String,
        text: String,
        translation: bool,
    },
    Phrase {
        reading: String,
        text: String,
    },
}
impl State {
    fn rebuild_language_preferences(&mut self) {
        for (word, entry) in &mut self.language_counts {
            entry.count = entry.count.max(1);
            if entry.active(word) {
                self.preferences.english.insert(word.clone(), entry.english);
            } else {
                self.preferences.english.remove(word);
            }
        }
        if self.language_counts.len() > 3000 {
            let mut oldest: Vec<_> = self
                .language_counts
                .iter()
                .map(|(word, entry)| (entry.used, word.clone()))
                .collect();
            oldest.sort();
            for (_, word) in oldest.into_iter().take(self.language_counts.len() - 2700) {
                self.language_counts.remove(&word);
                self.preferences.english.remove(&word);
                self.used.remove(&format!("language:{word}"));
            }
        }
    }
    pub fn apply(&mut self, change: Change) {
        match change {
            Change::Language {
                raw,
                english,
                explicit,
            } => {
                let raw = raw.to_ascii_lowercase();
                if raw.len() < 2 || !raw.bytes().all(|c| c.is_ascii_lowercase()) {
                    return;
                }
                let entry = self.language_counts.entry(raw.clone()).or_default();
                if entry.english != english {
                    *entry = LanguageEntry::default();
                }
                entry.english = english;
                entry.count = entry.count.saturating_add(1);
                entry.explicit |= explicit;
                entry.used = now();
                if entry.active(&raw) {
                    self.preferences.english.insert(raw.clone(), english);
                } else {
                    self.preferences.english.remove(&raw);
                }
                self.used.insert(format!("language:{raw}"), now());
                if self.language_counts.len() > 3000 {
                    self.rebuild_language_preferences();
                }
            }
            Change::Conversion {
                reading,
                text,
                translation,
            } => {
                if crate::time_candidates::is_dynamic_choice(&reading, &text) {
                    return;
                }
                if reading.chars().count() < 2 || text.is_empty() {
                    return;
                }
                if translation {
                    let count = self
                        .translations
                        .entry(reading.clone())
                        .or_default()
                        .entry(text)
                        .or_default();
                    *count = count.saturating_add(1);
                } else {
                    self.preferences.selected.insert(reading.clone(), text);
                }
                self.used.insert(format!("conversion:{reading}"), now());
            }
            Change::Phrase { reading, text } => {
                if crate::time_candidates::is_dynamic_choice(&reading, &text) {
                    return;
                }
                if reading.chars().count() < 3
                    || reading == text
                    || text.is_empty()
                    || text.contains(['\t', '\n', '\r'])
                {
                    return;
                }
                if let Some(old) = self
                    .phrases
                    .iter_mut()
                    .find(|p| p.reading == reading && p.text == text)
                {
                    old.count = old.count.saturating_add(1);
                    old.used = now();
                } else {
                    self.phrases.push(Phrase {
                        reading,
                        text,
                        count: 1,
                        used: now(),
                    });
                }
                if self.phrases.len() > 3000 {
                    self.phrases.sort_by_key(|p| std::cmp::Reverse(p.used));
                    self.phrases.truncate(2700);
                }
            }
        }
        if self.used.len() > 10000 {
            let mut oldest: Vec<_> = self.used.iter().map(|(k, t)| (k.clone(), *t)).collect();
            oldest.sort_by_key(|(_, t)| *t);
            for (key, _) in oldest.into_iter().take(1000) {
                self.remove_learned(&key);
            }
        }
    }
    pub fn remove_learned(&mut self, key: &str) {
        if let Some(prefix) = key.strip_prefix("prefix:") {
            self.prefix_model.prefixes.remove(prefix);
        }
        if let Some(raw) = key.strip_prefix("language:") {
            self.preferences.english.remove(raw);
            self.language_counts.remove(raw);
        }
        if let Some(reading) = key.strip_prefix("conversion:") {
            self.preferences.selected.remove(reading);
            self.translations.remove(reading);
        }
        if let Some(reading) = key.strip_prefix("phrase:") {
            self.phrases.retain(|phrase| phrase.reading != reading);
        }
        self.used.remove(key);
    }
    pub fn forget_since(&mut self, since: i64) -> usize {
        let keys: Vec<_> = self
            .used
            .iter()
            .filter(|(_, time)| **time >= since)
            .map(|(key, _)| key.clone())
            .collect();
        let count = keys.len();
        for key in keys {
            self.remove_learned(&key);
        }
        let old = self.phrases.len();
        self.phrases.retain(|p| p.used < since);
        count + old - self.phrases.len()
    }
    pub fn clear_learning(&mut self) {
        self.prefix_model.prefixes.clear();
        self.preferences.english.clear();
        self.preferences.selected.clear();
        self.language_counts.clear();
        self.translations.clear();
        self.phrases.clear();
        self.used.clear();
    }
}

impl Store {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }
    pub fn default_directory() -> PathBuf {
        if let Some(path) = std::env::var_os("MELTYPE_RUST_DATA_DIR") {
            return path.into();
        }
        #[cfg(windows)]
        {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
                .join("MeltypeRust")
        }
        #[cfg(target_os = "macos")]
        {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into()))
                .join("Library/Application Support/MeltypeRust")
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into()))
                        .join(".local/share")
                })
                .join("MeltypeRust")
        }
    }
    fn lock(&self) -> io::Result<Lock> {
        fs::create_dir_all(&self.directory)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.directory.join(".lock"))?;
        fs2::FileExt::lock_exclusive(&file)?;
        Ok(Lock(file))
    }
    pub fn settings(&self) -> io::Result<Settings> {
        match fs::read_to_string(self.directory.join("config.json")) {
            Ok(text) => Settings::parse(&text).map_err(io::Error::other),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let mut s = Settings::default();
                s.normalize();
                Ok(s)
            }
            Err(e) => Err(e),
        }
    }
    pub fn change_settings<T>(
        &self,
        f: impl FnOnce(&mut Settings) -> Result<T, String>,
    ) -> io::Result<T> {
        let _lock = self.lock()?;
        let mut settings = self.settings()?;
        let result = f(&mut settings).map_err(io::Error::other)?;
        atomic_write(
            &self.directory.join("config.json"),
            &serde_json::to_vec_pretty(&settings)?,
        )?;
        Ok(result)
    }
    pub fn load(&self) -> io::Result<State> {
        let path = self.directory.join("state.json");
        let mut state: State = match fs::read(&path) {
            Ok(bytes) => {
                let mut document: Value = serde_json::from_slice(&bytes)?;
                let object = document
                    .as_object_mut()
                    .ok_or_else(|| io::Error::other("invalid state document"))?;
                if !object.contains_key("prefix_model") {
                    object.insert(
                        "prefix_model".into(),
                        serde_json::to_value(self.legacy_prefix_model()?)?,
                    );
                }
                serde_json::from_value(document).map_err(io::Error::other)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => self.import_legacy(),
            Err(e) => Err(e),
        }?;
        state.rebuild_language_preferences();
        for (key, stats) in &state.prefix_model.prefixes {
            state.used.insert(format!("prefix:{key}"), stats.used());
        }
        Ok(state)
    }
    pub fn update<T>(&self, f: impl FnOnce(&mut State) -> io::Result<T>) -> io::Result<T> {
        let _lock = self.lock()?;
        let mut state = self.load()?;
        let result = f(&mut state)?;
        atomic_write(
            &self.directory.join("state.json"),
            &serde_json::to_vec_pretty(&state)?,
        )?;
        Ok(result)
    }
    pub fn record(&self, changes: Vec<Change>) -> io::Result<State> {
        self.update(|state| {
            for change in changes {
                state.apply(change);
            }
            Ok(state.clone())
        })
    }
    fn import_legacy(&self) -> io::Result<State> {
        let mut state = State {
            prefix_model: self.legacy_prefix_model()?,
            ..Default::default()
        };
        let read = |name: &str| -> io::Result<Value> {
            match fs::read(self.directory.join(name)) {
                Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Value::Null),
                Err(e) => Err(e),
            }
        };
        if let Some(languages) = read("languages.json")?.as_object() {
            for (raw, entry) in languages {
                let english = entry["English"].as_bool().unwrap_or(false);
                state.preferences.english.insert(raw.clone(), english);
                state.language_counts.insert(
                    raw.clone(),
                    LanguageEntry {
                        english,
                        count: entry["Count"].as_u64().unwrap_or(1).min(u32::MAX as u64) as u32,
                        explicit: entry["Explicit"] == true,
                        used: parse_time(&entry["Used"]),
                    },
                );
            }
        }
        if let Some(conversions) = read("conversions.json")?.as_object() {
            for (reading, entry) in conversions {
                if let Some(text) = entry["Text"].as_str() {
                    state
                        .preferences
                        .selected
                        .insert(reading.clone(), text.into());
                    state
                        .used
                        .insert(format!("conversion:{reading}"), parse_time(&entry["Used"]));
                }
            }
        }
        if let Some(translations) = read("translations.json")?.as_object() {
            for (reading, entries) in translations {
                if let Some(entries) = entries.as_object() {
                    for (word, count) in entries {
                        state
                            .translations
                            .entry(reading.clone())
                            .or_default()
                            .insert(
                                word.clone(),
                                count.as_u64().unwrap_or(1).min(u32::MAX as u64) as u32,
                            );
                    }
                }
            }
        }
        if let Some(snippets) = read("snippets.json")?.as_object() {
            for (name, text) in snippets {
                if let Some(text) = text.as_str() {
                    state.snippets.insert(name.clone(), text.into());
                }
            }
        }
        match fs::read(self.directory.join("userdict.txt")) {
            Ok(bytes) => {
                for (reading, word) in crate::user_dictionary::parse(&bytes)?.words {
                    state
                        .preferences
                        .user_dictionary
                        .entry(reading)
                        .or_default()
                        .push(word);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        if let Ok(text) = fs::read_to_string(self.directory.join("phrases.txt")) {
            for line in text.lines() {
                let p: Vec<_> = line.split('\t').collect();
                if p.len() >= 4 {
                    if let Ok(count) = p[2].parse() {
                        state.phrases.push(Phrase {
                            reading: p[0].into(),
                            text: p[1].into(),
                            count,
                            used: p[3]
                                .parse::<i64>()
                                .map(|ticks| ticks / 10_000_000 - 62_135_596_800)
                                .unwrap_or(0),
                        });
                    }
                }
            }
        }
        for (raw, entry) in &state.language_counts {
            state.used.insert(format!("language:{raw}"), entry.used);
        }
        Ok(state)
    }
    fn legacy_prefix_model(&self) -> io::Result<crate::prefix_learning::Model> {
        let path = self.directory.join("model.json");
        match fs::read(&path) {
            Ok(bytes) => match crate::prefix_learning::Model::parse(&bytes) {
                Ok(model) => Ok(model),
                Err(_) => {
                    let _ = atomic_write(&self.directory.join("model.json.broken"), &bytes);
                    Ok(Default::default())
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Default::default()),
            Err(e) => Err(e),
        }
    }
    pub fn backup(&self) -> io::Result<Vec<u8>> {
        let _lock = self.lock()?;
        let mut files = BTreeMap::new();
        for name in ROOT_FILES {
            let path = safe_path(&self.directory, name)?;
            if path.is_file() {
                files.insert((*name).to_owned(), STANDARD.encode(fs::read(path)?));
            }
        }
        let dictionaries = self.directory.join("dictionaries");
        if dictionaries.exists() {
            safe_path(&self.directory, "dictionaries")?;
            for entry in fs::read_dir(dictionaries)? {
                let entry = entry?;
                let name = format!("dictionaries/{}", entry.file_name().to_string_lossy());
                if restorable_name(&name) {
                    files.insert(
                        name.clone(),
                        STANDARD.encode(fs::read(safe_path(&self.directory, &name)?)?),
                    );
                }
            }
        }
        Ok(serde_json::to_vec_pretty(
            &json!({"format":"meltype-backup","version":1,"app":env!("CARGO_PKG_VERSION"),"created":chrono::Utc::now().to_rfc3339(),"files":files}),
        )?)
    }
    pub fn inspect_backup(bytes: &[u8]) -> io::Result<Value> {
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(io::Error::other("backup too large"));
        }
        let value: Value = serde_json::from_slice(bytes)?;
        if value["format"] != "meltype-backup"
            || value["version"] != 1
            || !value["files"].is_object()
        {
            return Err(io::Error::other("invalid backup format or version"));
        }
        Ok(value)
    }
    pub fn restore(&self, bytes: &[u8]) -> io::Result<usize> {
        let value = Self::inspect_backup(bytes)?;
        let _lock = self.lock()?;
        let files = self.backup_entries(&value)?;
        // Validate every entry before the first write; preserve old contents.
        for (path, bytes) in &files {
            if path.exists() {
                let backup = path.with_file_name(format!(
                    "{}.before-restore",
                    path.file_name().unwrap().to_string_lossy()
                ));
                atomic_write(&backup, &fs::read(path)?)?;
            }
            atomic_write(path, bytes)?;
        }
        Ok(files.len())
    }
    /// Metadata for a restoration decision, without exposing encoded user data.
    /// Use the same target/path/base64 validation as restore, without writing.
    pub fn preview_backup(&self, bytes: &[u8]) -> io::Result<Value> {
        let value = Self::inspect_backup(bytes)?;
        let files = self.backup_entries(&value)?;
        let mut categories = Vec::new();
        let names: Vec<_> = value["files"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|name| restorable_name(name))
            .map(String::as_str)
            .collect();
        for (category, present) in [
            ("設定", names.contains(&"config.json")),
            (
                "学習データ",
                names.iter().any(|n| {
                    matches!(
                        *n,
                        "state.json"
                            | "model.json"
                            | "conversions.json"
                            | "translations.json"
                            | "languages.json"
                            | "phrases.txt"
                    )
                }),
            ),
            (
                "ユーザー辞書",
                names.contains(&"state.json")
                    || names
                        .iter()
                        .any(|n| *n == "userdict.txt" || n.starts_with("dictionaries/")),
            ),
            (
                "定型文",
                names.contains(&"state.json") || names.contains(&"snippets.json"),
            ),
        ] {
            if present {
                categories.push(category);
            }
        }
        let created = value["created"]
            .as_str()
            .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
            .map(|date| date.to_rfc3339());
        Ok(json!({"created":created,"fileCount":files.len(),
            "bytes":files.iter().map(|(_,bytes)|bytes.len() as u64).sum::<u64>(),
            "categories":categories,"ignoredFiles":value["files"].as_object().unwrap().len()-files.len()}))
    }
    fn backup_entries(&self, value: &Value) -> io::Result<Vec<(PathBuf, Vec<u8>)>> {
        value["files"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(n, _)| restorable_name(n))
            .map(|(name, text)| {
                let path = safe_path(&self.directory, name)?;
                let bytes = STANDARD
                    .decode(
                        text.as_str()
                            .ok_or_else(|| io::Error::other("invalid backup bytes"))?,
                    )
                    .map_err(io::Error::other)?;
                Ok((path, bytes))
            })
            .collect()
    }
}
fn parse_time(v: &Value) -> i64 {
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp())
        .unwrap_or(0)
}
const ROOT_FILES: &[&str] = &[
    "config.json",
    "state.json",
    "model.json",
    "conversions.json",
    "translations.json",
    "languages.json",
    "userdict.txt",
    "snippets.json",
    "phrases.txt",
];
pub fn restorable_name(name: &str) -> bool {
    if ROOT_FILES.contains(&name) {
        return true;
    }
    let Some(file) = name.strip_prefix("dictionaries/") else {
        return false;
    };
    if file.starts_with('.')
        || !file.ends_with(".txt")
        || file.len() > 104
        || file
            .chars()
            .any(|c| c.is_control() || "\\/:*?\"<>|".contains(c))
    {
        return false;
    }
    let base = file.split('.').next().unwrap_or("").to_ascii_lowercase();
    !["con", "prn", "aux", "nul"].contains(&base.as_str())
        && !(base.len() == 4
            && (base.starts_with("com") || base.starts_with("lpt"))
            && base.as_bytes()[3].is_ascii_digit())
}
fn safe_path(root: &Path, name: &str) -> io::Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in Path::new(name).components() {
        match part {
            std::path::Component::Normal(p) => path.push(p),
            _ => return Err(io::Error::other("unsafe data path")),
        };
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(io::Error::other("symlink in data path"));
        }
    }
    Ok(path)
}
