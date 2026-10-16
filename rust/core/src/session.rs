// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use crate::{dictionary::Dictionary, romaji};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEdit {
    pub delete_before: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub text: String,
    pub converting: bool,
    pub selected_index: usize,
    pub selected_clause: usize,
    pub hint: String,
    pub candidates: Vec<String>,
    pub clauses: Vec<String>,
    pub meaning: Option<String>,
    pub suggestion: Option<String>,
    pub predictions: Vec<String>,
    pub selected_prediction: i32,
    pub typed: Option<String>,
    pub notes: Vec<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResult {
    pub consumed: bool,
    pub commits: Vec<TextEdit>,
    pub view: Option<View>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub english: BTreeMap<String, bool>,
    pub selected: BTreeMap<String, String>,
    pub user_dictionary: BTreeMap<String, Vec<String>>,
}

#[derive(Default)]
pub struct Session {
    raw: String,
    candidates: Vec<String>,
    selected: usize,
    explicit: Option<String>,
    pub direct: bool,
    pub code_input: bool,
    pub private: bool,
    pub preferences: Preferences,
}

impl Session {
    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn reading(&self) -> String {
        romaji::hiragana(&self.raw, true)
    }
    pub fn is_converting(&self) -> bool {
        !self.candidates.is_empty()
    }
    pub fn display(&self, final_input: bool) -> String {
        if let Some(text) = &self.explicit {
            return text.clone();
        }
        if let Some(text) = self.candidates.get(self.selected) {
            return text.clone();
        }
        match self.preferences.english.get(&self.raw.to_ascii_lowercase()) {
            Some(true) => self.raw.clone(),
            Some(false) => romaji::hiragana(&self.raw, final_input),
            None => Dictionary::bundled().mixed(&self.raw, final_input),
        }
    }
    pub fn result(&self, consumed: bool) -> SessionResult {
        SessionResult {
            consumed,
            commits: vec![],
            view: if self.raw.is_empty() {
                None
            } else {
                Some(View {
                    text: self.display(false),
                    converting: self.is_converting(),
                    selected_index: self.selected,
                    selected_clause: 0,
                    hint: String::new(),
                    clauses: vec![],
                    candidates: self.candidates.clone(),
                    meaning: None,
                    suggestion: None,
                    predictions: vec![],
                    selected_prediction: -1,
                    typed: None,
                    notes: vec![None; self.candidates.len()],
                })
            },
        }
    }
    pub fn reset(&mut self) {
        self.raw.clear();
        self.candidates.clear();
        self.explicit = None;
        self.selected = 0;
    }
    pub fn commit(&mut self) -> SessionResult {
        let text = self.display(true);
        if !self.private && self.is_converting() {
            self.preferences
                .selected
                .insert(self.reading(), text.clone());
        }
        self.reset();
        SessionResult {
            consumed: !text.is_empty(),
            commits: if text.is_empty() {
                vec![]
            } else {
                vec![TextEdit {
                    delete_before: 0,
                    text,
                }]
            },
            view: None,
        }
    }
    pub fn select(&mut self, index: usize) -> SessionResult {
        if index >= self.candidates.len() {
            return self.result(false);
        }
        self.selected = index;
        self.commit()
    }
    pub fn begin_conversion(&mut self, external: Vec<String>) {
        if self.raw.is_empty() || self.is_converting() {
            return;
        }
        let reading = self.reading();
        let mut candidates = self
            .preferences
            .user_dictionary
            .get(&reading)
            .cloned()
            .unwrap_or_default();
        candidates.extend(external);
        candidates.extend(Dictionary::bundled().candidates(&self.display(true)));
        candidates.extend(Dictionary::bundled().candidates(&reading));
        candidates.push(self.raw.clone());
        let mut unique = Vec::new();
        for candidate in candidates {
            if !candidate.is_empty() && !unique.contains(&candidate) {
                unique.push(candidate);
            }
        }
        if let Some(learned) = self.preferences.selected.get(&reading) {
            if let Some(i) = unique.iter().position(|s| s == learned) {
                let value = unique.remove(i);
                unique.insert(0, value);
            }
        }
        self.candidates = unique;
        self.selected = 0;
        self.explicit = None;
    }
    /// Virtual keys follow the existing native ABI; modifiers: shift=1, ctrl=2,
    /// alt=4, command=8. Platform shortcuts commit first and pass through.
    pub fn key(&mut self, vk: i32, ch: i32, modifiers: i32) -> SessionResult {
        if ch < 0 || char::from_u32(ch as u32).is_none() {
            return self.result(false);
        }
        if modifiers & 14 != 0 || self.direct || self.private || self.code_input {
            let mut result = self.commit();
            result.consumed = false;
            return result;
        }
        match vk {
            0x1b => {
                if self.raw.is_empty() {
                    return self.result(false);
                }
                if self.is_converting() {
                    self.candidates.clear();
                    self.explicit = None;
                } else {
                    self.reset();
                }
                self.result(true)
            }
            0x08 => {
                if self.raw.is_empty() {
                    return self.result(false);
                }
                self.candidates.clear();
                self.explicit = None;
                self.raw.pop();
                self.result(true)
            }
            0x0d => self.commit(),
            0x20 if !self.raw.is_empty() => {
                if self.is_converting() {
                    self.cycle(if modifiers & 1 != 0 { -1 } else { 1 });
                } else {
                    self.begin_conversion(vec![]);
                }
                self.result(true)
            }
            0x26 | 0x28 | 0x09 if self.is_converting() => {
                self.cycle(if vk == 0x26 || modifiers & 1 != 0 {
                    -1
                } else {
                    1
                });
                self.result(true)
            }
            0x75..=0x79 if !self.raw.is_empty() => {
                self.candidates.clear();
                self.explicit = Some(match vk {
                    0x75 => romaji::hiragana(&self.raw, true),
                    0x76 | 0x77 => romaji::katakana(&self.reading()),
                    0x78 => self
                        .raw
                        .chars()
                        .map(|c| {
                            if c.is_ascii_graphic() {
                                char::from_u32(c as u32 + 0xfee0).unwrap()
                            } else {
                                c
                            }
                        })
                        .collect(),
                    _ => self.raw.clone(),
                });
                if !self.private {
                    self.preferences
                        .english
                        .insert(self.raw.to_ascii_lowercase(), vk >= 0x78);
                }
                self.result(true)
            }
            _ if ch >= 0x20 && ch != 0x7f => {
                // Retain the old commit when a new character starts after conversion.
                let commits = if self.is_converting() {
                    self.commit().commits
                } else {
                    vec![]
                };
                self.explicit = None;
                if self.raw.chars().count() >= 1024 {
                    let mut result = self.commit();
                    result.consumed = false;
                    return result;
                }
                self.raw.push(char::from_u32(ch as u32).unwrap());
                let mut result = self.result(true);
                result.commits = commits;
                result
            }
            _ => {
                let mut result = self.commit();
                result.consumed = false;
                result
            }
        }
    }
    fn cycle(&mut self, direction: i32) {
        self.selected =
            (self.selected as i32 + direction).rem_euclid(self.candidates.len() as i32) as usize;
    }
}
