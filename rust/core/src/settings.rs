// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Lossless settings document: upstream names and unknown future fields survive
//! edits. Defaults originate from Settings.cs at the migration baseline.
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Settings(pub Map<String, Value>);

const SHARED: &[&str] = &[
    "Profiles",
    "ActiveProfile",
    "SettingsVersion",
    "WelcomeShown",
    "Enabled",
    "JapaneseKeyboardOnly",
    "AllowInjectedInput",
    "FileLog",
    "LogTypedText",
    "AutoUpdate",
    "Mode",
    "TsfIntroduced",
    "TsfOffered",
    "SettingsShowHelp",
    "SettingsWheelChangesValue",
];
impl Default for Settings {
    fn default() -> Self {
        Self(
            serde_json::from_str(include_str!("../data/settings-defaults.json"))
                .expect("bundled settings"),
        )
    }
}
impl Settings {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut loaded: Map<String, Value> = json5::from_str(text).map_err(|e| e.to_string())?;
        let mut defaults = Self::default();
        let version = loaded
            .get("SettingsVersion")
            .and_then(Value::as_i64)
            .unwrap_or(1);
        if version < 2 {
            if loaded.get("IdleFlushMs") == Some(&json!(400)) {
                loaded.insert("IdleFlushMs".into(), json!(700));
            }
            if loaded.get("MaxHoldMs") == Some(&json!(1200)) {
                loaded.insert("MaxHoldMs".into(), json!(2500));
            }
        }
        if version < 4 {
            let rules = loaded.entry("AppRules").or_insert_with(|| json!([]));
            if let Some(rules) = rules.as_array_mut() {
                for rule in defaults.0["AppRules"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| r["Profile"] == "Code")
                {
                    if let Some(old) = rules.iter_mut().find(|r| {
                        r["Process"]
                            .as_str()
                            .unwrap_or("")
                            .eq_ignore_ascii_case(rule["Process"].as_str().unwrap())
                    }) {
                        old["Profile"] = json!("Code");
                    } else {
                        rules.push(rule.clone());
                    }
                }
            }
        } else if version < 5 {
            if let Some(rules) = loaded.get_mut("AppRules").and_then(Value::as_array_mut) {
                if !rules.iter().any(|r| {
                    r["Process"]
                        .as_str()
                        .unwrap_or("")
                        .eq_ignore_ascii_case("maya.exe")
                }) {
                    rules.push(json!({"Process":"maya.exe","Enabled":true,"Profile":"Code"}));
                }
            }
        }
        loaded.insert("SettingsVersion".into(), json!(5));
        defaults.0.extend(loaded);
        defaults.normalize();
        Ok(defaults)
    }
    pub fn boolean(&self, name: &str) -> bool {
        self.0.get(name).and_then(Value::as_bool).unwrap_or(false)
    }
    pub fn integer(&self, name: &str) -> i64 {
        self.0.get(name).and_then(Value::as_i64).unwrap_or(0)
    }
    pub fn text(&self, name: &str) -> &str {
        self.0.get(name).and_then(Value::as_str).unwrap_or("")
    }
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), String> {
        if let Some(default) = Self::default().0.get(name) {
            if !(default.is_boolean() && value.is_boolean()
                || default.is_number() && value.is_i64()
                || default.is_string() && value.is_string()
                || default.is_array() && value.is_array())
            {
                return Err(format!("invalid type for {name}"));
            }
        }
        self.0.insert(name.into(), value);
        self.normalize();
        Ok(())
    }
    pub fn normalize(&mut self) {
        let defaults = Self::default();
        for (name, default) in &defaults.0 {
            let value = self
                .0
                .entry(name.clone())
                .or_insert_with(|| default.clone());
            if value.is_null() {
                *value = default.clone();
            }
        }
        for (name, min, max) in [
            ("JapaneseThreshold", 2, 20),
            ("MaxPendingKeys", 2, 12),
            ("IdleFlushMs", 100, 3000),
            ("MaxHoldMs", 200, 5000),
            ("SessionIdleMs", 300, 60000),
            ("FeedbackWindowMs", 500, 30000),
            ("ImeTimeoutMs", 50, 2000),
        ] {
            let value = self
                .0
                .get(name)
                .and_then(Value::as_i64)
                .unwrap_or(defaults.integer(name))
                .clamp(min, max);
            self.0.insert(name.into(), json!(value));
        }
        for key in ["AppRules", "AppKinds", "Profiles"] {
            if !self.0[key].is_array() {
                self.0.insert(key.into(), json!([]));
            }
        }
        if let Some(rules) = self.0.get_mut("AppRules").and_then(Value::as_array_mut) {
            let mut seen = HashSet::new();
            rules.reverse();
            rules.retain(|r| {
                r["Process"]
                    .as_str()
                    .is_some_and(|s| !s.trim().is_empty() && seen.insert(s.trim().to_lowercase()))
            });
            rules.reverse();
            for r in rules {
                r["Process"] = json!(r["Process"].as_str().unwrap().trim());
            }
        }
        let name = self.text("ActiveProfile").trim().to_owned();
        let name = if name.is_empty() {
            "標準".to_owned()
        } else {
            name
        };
        self.0.insert("ActiveProfile".into(), json!(name));
        let profiles = self.0["Profiles"].as_array().unwrap();
        let mut seen = HashSet::new();
        let mut profiles: Vec<_> = profiles
            .iter()
            .filter_map(|p| {
                let name = p["Name"].as_str()?.trim();
                if name.is_empty() || !seen.insert(name.to_owned()) {
                    return None;
                }
                Some(json!({"Name":name,"Values":p["Values"]}))
            })
            .collect();
        if !profiles.iter().any(|p| p["Name"] == name) {
            profiles.insert(0, json!({"Name":name,"Values":null}));
        }
        self.0.insert("Profiles".into(), json!(profiles));
    }
    fn profile_values(&self) -> Value {
        Value::Object(
            self.0
                .iter()
                .filter(|(k, _)| !SHARED.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }
    pub fn add_profile(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("empty profile name".into());
        }
        if self.0["Profiles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["Name"] == name)
        {
            return Err("profile already exists".into());
        }
        let values = self.profile_values();
        self.0["Profiles"]
            .as_array_mut()
            .unwrap()
            .push(json!({"Name":name,"Values":values}));
        Ok(())
    }
    pub fn switch_profile(&mut self, name: &str) -> Result<(), String> {
        let target = self.0["Profiles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["Name"] == name)
            .cloned()
            .ok_or("unknown profile")?;
        let active = self.text("ActiveProfile").to_owned();
        let values = self.profile_values();
        if let Some(current) = self.0["Profiles"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["Name"] == active)
        {
            current["Values"] = values;
        }
        let defaults = Self::default();
        for (key, value) in defaults
            .0
            .iter()
            .filter(|(k, _)| !SHARED.contains(&k.as_str()))
        {
            self.0.insert(key.clone(), value.clone());
        }
        if let Some(values) = target["Values"].as_object() {
            for (key, value) in values.iter().filter(|(k, _)| !SHARED.contains(&k.as_str())) {
                self.0.insert(key.clone(), value.clone());
            }
        }
        self.0.insert("ActiveProfile".into(), json!(name));
        self.normalize();
        Ok(())
    }
    pub fn delete_profile(&mut self, name: &str) -> Result<(), String> {
        if name == self.text("ActiveProfile") {
            return Err("switch profiles before deleting the active profile".into());
        }
        self.0["Profiles"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p["Name"] != name);
        Ok(())
    }
    pub fn for_app(&self, process: &str) -> Self {
        let mut settings = self.clone();
        let stem = |s: &str| s.to_lowercase().trim_end_matches(".exe").to_owned();
        let rule = self.0["AppRules"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|r| {
                r["Process"]
                    .as_str()
                    .is_some_and(|s| stem(s) == stem(process))
            });
        if let Some(rule) = rule {
            if rule["Enabled"] == false {
                settings.0.insert("Enabled".into(), json!(false));
            }
            settings.0.insert(
                "AppProfile".into(),
                rule.get("Profile").cloned().unwrap_or(json!("General")),
            );
            if let Some(kind) = self.0["AppKinds"]
                .as_array()
                .unwrap()
                .iter()
                .find(|k| k["Name"] == rule["Kind"] && k["Name"].is_string())
            {
                settings.0.insert(
                    "AppProfile".into(),
                    kind.get("Base").cloned().unwrap_or(json!("General")),
                );
                for key in ["DetectionLevel", "LiveConversion", "StartInEnglish"] {
                    if let Some(v) = kind.get(key).filter(|v| !v.is_null()) {
                        settings.0.insert(key.into(), v.clone());
                    }
                }
            }
        }
        if settings.text("AppProfile") == "Game" {
            settings.0.insert("Enabled".into(), json!(false));
        }
        settings
    }
}
