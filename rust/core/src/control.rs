// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Management operations shared by CLI and platform settings clients.
use crate::{
    storage::{atomic_write, Store},
    user_dictionary,
};
use serde_json::{json, Value};
use std::{io, path::Path};
fn field<'a>(r: &'a Value, n: &str) -> io::Result<&'a str> {
    r[n].as_str()
        .ok_or_else(|| io::Error::other(format!("missing {n}")))
}
pub fn execute(store: &Store, r: &Value) -> io::Result<Value> {
    match field(r, "op")? {
        "settings_get" => Ok(serde_json::to_value(store.settings()?)?),
        "settings_set" => {
            let name = field(r, "name")?;
            let value = r
                .get("value")
                .ok_or_else(|| io::Error::other("missing value"))?
                .clone();
            store.change_settings(|s| s.set(name, value))?;
            Ok(json!({"saved":true}))
        }
        "profile_add" | "profile_switch" | "profile_delete" => {
            let name = field(r, "name")?;
            store.change_settings(|s| match field(r, "op").unwrap() {
                "profile_add" => s.add_profile(name),
                "profile_switch" => s.switch_profile(name),
                _ => s.delete_profile(name),
            })?;
            Ok(json!({"saved":true}))
        }
        "dictionary_list" => Ok(serde_json::to_value(
            store.load()?.preferences.user_dictionary,
        )?),
        "dictionary_set" => {
            let reading = user_dictionary::hiragana(field(r, "reading")?.trim());
            let word = field(r, "word")?.trim();
            if !user_dictionary::valid(&reading, word) {
                return Err(io::Error::other("invalid reading or word"));
            }
            store.update(|s| {
                let entries = s.preferences.user_dictionary.entry(reading).or_default();
                if !entries.iter().any(|w| w == word) {
                    entries.push(word.into());
                }
                Ok(())
            })?;
            Ok(json!({"saved":true}))
        }
        "dictionary_remove" => {
            let reading = field(r, "reading")?;
            let word = r["word"].as_str();
            store.update(|s| {
                if let Some(word) = word {
                    if let Some(words) = s.preferences.user_dictionary.get_mut(reading) {
                        words.retain(|w| w != word);
                    }
                } else {
                    s.preferences.user_dictionary.remove(reading);
                }
                Ok(())
            })?;
            Ok(json!({"saved":true}))
        }
        "dictionary_import" => {
            let import = user_dictionary::parse(&std::fs::read(field(r, "path")?)?)?;
            let count = import.words.len();
            store.update(|s| {
                for (reading, word) in import.words {
                    let entries = s.preferences.user_dictionary.entry(reading).or_default();
                    if !entries.contains(&word) {
                        entries.push(word);
                    }
                }
                Ok(())
            })?;
            Ok(json!({"imported":count,"skipped":import.skipped,"encoding":import.encoding}))
        }
        "dictionary_export" => {
            let words = store
                .load()?
                .preferences
                .user_dictionary
                .into_iter()
                .flat_map(|(reading, words)| {
                    words.into_iter().map(move |word| (reading.clone(), word))
                });
            atomic_write(
                Path::new(field(r, "path")?),
                &user_dictionary::export(words),
            )?;
            Ok(json!({"saved":true}))
        }
        "snippets_list" => Ok(serde_json::to_value(store.load()?.snippets)?),
        "snippet_set" => {
            let name = field(r, "name")?;
            let text = field(r, "text")?;
            if name.is_empty()
                || !name.chars().all(|c| c.is_ascii_graphic() && c != ';')
                || text.is_empty()
            {
                return Err(io::Error::other("invalid snippet"));
            }
            store.update(|s| {
                s.snippets
                    .insert(name.into(), text.replace("\r\n", "\n").replace('\r', "\n"));
                Ok(())
            })?;
            Ok(json!({"saved":true}))
        }
        "snippet_remove" => {
            store.update(|s| {
                s.snippets.remove(field(r, "name")?);
                Ok(())
            })?;
            Ok(json!({"saved":true}))
        }
        "learning_list" => {
            let s = store.load()?;
            Ok(
                json!({"languages":s.language_counts,"conversions":s.preferences.selected,"translations":s.translations,"phrases":s.phrases}),
            )
        }
        "learning_clear" => {
            store.update(|s| {
                s.clear_learning();
                Ok(())
            })?;
            Ok(json!({"saved":true}))
        }
        "learning_forget_since" => {
            let since = r["since"]
                .as_i64()
                .ok_or_else(|| io::Error::other("since must be a UTC Unix timestamp"))?;
            let count = store.update(|s| Ok(s.forget_since(since)))?;
            Ok(json!({"removed":count}))
        }
        "learning_remove" => {
            let key = field(r, "key")?;
            store.update(|s| {
                s.remove_learned(key);
                Ok(())
            })?;
            Ok(json!({"saved":true}))
        }
        "backup_create" => {
            atomic_write(Path::new(field(r, "path")?), &store.backup()?)?;
            Ok(json!({"saved":true}))
        }
        "backup_inspect" => Store::inspect_backup(&std::fs::read(field(r, "path")?)?),
        "backup_restore" => {
            let count = store.restore(&std::fs::read(field(r, "path")?)?)?;
            Ok(json!({"restored":count}))
        }
        "diagnostics" => Ok(
            json!({"version":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"dataDirectory":store.directory,"settingsReadable":store.settings().is_ok(),"stateReadable":store.load().is_ok()}),
        ),
        _ => Err(io::Error::other("unknown management operation")),
    }
}
