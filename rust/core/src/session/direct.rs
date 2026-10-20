// SPDX-License-Identifier: GPL-3.0-or-later
// Port of CompositionController direct detection and MeltypeSession word lifetime.
use super::{romaji, Session, SessionResult, TextEdit};

impl Session {
    pub fn set_direct(&mut self, enabled: bool) {
        if self.direct != enabled {
            self.forget_direct_input();
            self.direct = enabled;
        }
    }
    pub fn forget_direct_input(&mut self) {
        self.direct_pending.clear();
        self.direct_last_key_ms = None;
        self.direct_english_word = false;
    }
    pub(super) fn direct_timeout(&mut self, now_ms: u64) -> Option<TextEdit> {
        if !self.direct
            || !self.direct_detection
            || self.private
            || !self.raw.is_empty()
            || !self.settings.boolean("Enabled")
            || !self.settings.boolean("DirectModeAutoDetect")
        {
            self.forget_direct_input();
            return None;
        }
        let expired = self
            .direct_last_key_ms
            .and_then(|last| now_ms.checked_sub(last))
            .is_some_and(|elapsed| elapsed >= 700);
        if !expired || self.direct_pending.is_empty() {
            return None;
        }
        let held = std::mem::take(&mut self.direct_pending);
        self.direct_last_key_ms = None;
        let keys: Vec<_> = held
            .bytes()
            .map(|b| b.to_ascii_uppercase() as i32)
            .collect();
        let part = held.rsplit('-').next().unwrap_or("");
        let part_keys: Vec<_> = part
            .bytes()
            .map(|b| b.to_ascii_uppercase() as i32)
            .collect();
        let decision = crate::detection::evaluate_with_model(
            part,
            &part_keys,
            true,
            &self.settings,
            Some(&self.state.prefix_model),
        );
        if decision.verdict == crate::detection::Verdict::Japanese
            && self.context_is_fresh
            && self.can_delete
            && !self.code_input
            && self.before.ends_with(&held)
            && self.settings.text("DetectionLevel") != "Manual"
        {
            self.before.truncate(self.before.len() - held.len());
            self.raw = held.clone();
            self.typed_keys = held
                .chars()
                .zip(keys)
                .map(|(c, vk)| (vk, c, false))
                .collect();
            self.direct = false;
            self.direct_english_word = false;
            self.refresh_kana();
            Some(TextEdit {
                delete_before: held.encode_utf16().count(),
                text: String::new(),
                expect: Some(held),
            })
        } else {
            self.direct_english_word = true;
            self.correctable.clear();
            None
        }
    }
    pub(super) fn detect_direct(
        &mut self,
        vk: i32,
        ch: i32,
        modifiers: i32,
    ) -> Option<SessionResult> {
        if !self.direct
            || !self.direct_detection
            || !self.context_is_fresh
            || !self.can_delete
            || self.private
            || self.code_input
            || !self.raw.is_empty()
            || !self.settings.boolean("Enabled")
            || !self.settings.boolean("DirectModeAutoDetect")
            || self.settings.text("DetectionLevel") == "Manual"
            || modifiers & 15 != 0
        {
            return None;
        }
        self.context_is_fresh = false;
        let c = char::from_u32(ch as u32)?;
        let boundary_character = |c: char| !c.is_ascii_alphanumeric() && !"@_".contains(c);
        let continuing =
            !self.direct_pending.is_empty() && self.before.ends_with(&self.direct_pending);
        if continuing && c == '-' {
            let part = self.direct_pending.rsplit('-').next().unwrap_or("");
            let fragment = romaji::analyze(part, true);
            if !part.is_empty() && fragment.valid && fragment.partial.is_empty() {
                let prefix = !self.direct_pending.contains('-') && Self::hyphen_prefix(part);
                self.direct_pending.push('-');
                self.direct_last_key_ms = Some(self.key_time_ms);
                if prefix {
                    return None;
                }
                let typed = std::mem::take(&mut self.direct_pending);
                self.direct_last_key_ms = None;
                let expected = &typed[..typed.len() - 1];
                self.before.truncate(self.before.len() - expected.len());
                self.raw = typed.clone();
                self.typed_keys = typed
                    .chars()
                    .map(|c| {
                        (
                            if c == '-' {
                                0xbd
                            } else {
                                c.to_ascii_uppercase() as i32
                            },
                            c,
                            false,
                        )
                    })
                    .collect();
                self.direct = false;
                self.direct_english_word = false;
                self.refresh_kana();
                let mut result = self.result(true);
                result.commits.push(TextEdit {
                    delete_before: expected.encode_utf16().count(),
                    text: String::new(),
                    expect: Some(expected.into()),
                });
                return Some(result);
            }
        }
        if boundary_character(c) {
            let was_checking = continuing;
            self.forget_direct_input();
            self.direct_english_word = was_checking;
            return None;
        }
        if !continuing
            && self
                .before
                .chars()
                .next_back()
                .is_some_and(boundary_character)
        {
            self.forget_direct_input();
        }
        if self.direct_english_word {
            return None;
        }
        if !c.is_ascii_lowercase() {
            self.direct_pending.clear();
            self.direct_last_key_ms = None;
            return None;
        }
        if !self.before.ends_with(&self.direct_pending) {
            self.forget_direct_input();
        }
        let prefix = self.direct_pending.clone();
        let boundary = &self.before[..self.before.len() - prefix.len()];
        let existing_letters = boundary
            .chars()
            .rev()
            .take_while(char::is_ascii_alphabetic)
            .count();
        let token_boundary = &boundary[..boundary.len() - existing_letters];
        // An identifier, command, path or mention never becomes a Japanese word.
        if token_boundary
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric() || "_@$/\\.:+".contains(c))
            || prefix.len() >= self.settings.integer("MaxPendingKeys") as usize
            || prefix.bytes().any(|c| c.is_ascii_uppercase())
        {
            self.direct_english_word = true;
            return None;
        }
        let typed = format!("{prefix}{c}");
        let mut keys: Vec<_> = prefix
            .bytes()
            .map(|b| b.to_ascii_uppercase() as i32)
            .collect();
        keys.push(if vk == 0 {
            c.to_ascii_uppercase() as i32
        } else {
            vk
        });
        let part = typed.rsplit('-').next().unwrap_or("");
        let part_keys: Vec<_> = part
            .bytes()
            .map(|b| b.to_ascii_uppercase() as i32)
            .collect();
        let mut result = crate::detection::evaluate_with_model(
            part,
            &part_keys,
            false,
            &self.settings,
            Some(&self.state.prefix_model),
        );
        if self.hyphenated_word(&typed) {
            result.verdict = crate::detection::Verdict::English;
        }
        let fragment = romaji::analyze(part, true);
        if result.verdict == crate::detection::Verdict::English
            && typed.len() <= 2
            && fragment.valid
            && !fragment.partial.is_empty()
        {
            result.verdict = crate::detection::Verdict::Undecided;
        }
        if self.preferences.english.get(&typed) == Some(&true) {
            result.verdict = crate::detection::Verdict::English;
        }
        self.direct_english_word = matches!(
            result.verdict,
            crate::detection::Verdict::English | crate::detection::Verdict::Unknown
        );
        if result.verdict != crate::detection::Verdict::Japanese {
            if !self.direct_english_word {
                self.direct_pending = typed;
                self.direct_last_key_ms = Some(self.key_time_ms);
            } else {
                self.direct_pending.clear();
                self.direct_last_key_ms = None;
            }
            return None;
        }
        self.direct_pending.clear();
        self.direct_last_key_ms = None;
        self.direct = false;
        self.direct_english_word = false;
        self.composition_kana = self.settings.text("InputStyle") == "Both"
            && result
                .contributions
                .iter()
                .any(|c| c.source == "Kana" && c.japanese > 0)
            && result
                .contributions
                .iter()
                .all(|c| c.source == "Kana" || c.japanese == 0);
        self.raw = typed;
        self.typed_keys = self
            .raw
            .chars()
            .zip(keys)
            .map(|(c, vk)| (vk, c, false))
            .collect();
        self.before.truncate(boundary.len());
        self.refresh_kana();
        let mut result = self.result(true);
        if !prefix.is_empty() {
            result.commits.push(TextEdit {
                delete_before: prefix.encode_utf16().count(),
                text: String::new(),
                expect: Some(prefix),
            });
        }
        Some(result)
    }
    fn hyphen_prefix(word: &str) -> bool {
        [
            "e", "x", "re", "co", "ex", "non", "anti", "semi", "multi", "pre", "sub", "post",
            "mid", "self", "well",
        ]
        .contains(&word)
    }
    fn hyphenated_word(&self, word: &str) -> bool {
        if [
            "bio-break",
            "co-op",
            "re-do",
            "x-ray",
            "t-shirt",
            "wi-fi",
            "hi-fi",
            "lo-fi",
            "sci-fi",
            "e-book",
            "e-sports",
            "k-pop",
            "j-pop",
            "j-rock",
            "j-core",
            "p-hub",
            "talk-admin",
            "r-18",
            "sub-6",
            "gpt-6.7",
            "a-z",
            "u-turn",
            "check-in",
            "log-in",
            "sign-in",
            "add-on",
            "plug-in",
            "built-in",
            "follow-up",
            "set-up",
            "pop-up",
            "drop-down",
            "blu-ray",
        ]
        .contains(&word)
        {
            return true;
        }
        word.split_once('-').is_some_and(|(head, rest)| {
            Self::hyphen_prefix(head)
                && rest.len() >= 3
                && rest.bytes().all(|b| b.is_ascii_lowercase())
                && self.dictionary().known_english(rest)
        })
    }
}
