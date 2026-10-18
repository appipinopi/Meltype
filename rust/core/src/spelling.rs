// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Scoped, already-resolved host spelling evidence. Never invokes host code.
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    marker::PhantomData,
    rc::Rc,
};
#[derive(Default)]
struct Evidence {
    active: bool,
    facts: HashMap<String, bool>,
    missing: BTreeSet<String>,
}
thread_local! { static EVIDENCE:RefCell<Evidence> = RefCell::default(); }
pub struct Scope {
    previous: Option<Evidence>,
    _thread: PhantomData<Rc<()>>,
}
impl Scope {
    pub fn disabled() -> Self {
        let previous = EVIDENCE.with(|s| s.replace(Evidence::default()));
        Self {
            previous: Some(previous),
            _thread: PhantomData,
        }
    }
    pub fn new(facts: HashMap<String, bool>) -> Self {
        let previous = EVIDENCE.with(|s| {
            s.replace(Evidence {
                active: true,
                facts,
                missing: BTreeSet::new(),
            })
        });
        Self {
            previous: Some(previous),
            _thread: PhantomData,
        }
    }
    pub fn missing(&self) -> Vec<String> {
        EVIDENCE.with(|s| s.borrow().missing.iter().cloned().collect())
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            EVIDENCE.with(|s| s.replace(previous));
        }
    }
}
pub fn active() -> bool {
    EVIDENCE.with(|s| s.borrow().active)
}
pub fn is_word(raw: &str) -> bool {
    if raw.len() < 2 || !raw.bytes().all(|b| b.is_ascii_alphabetic()) {
        return false;
    }
    EVIDENCE.with(|s| {
        let mut evidence = s.borrow_mut();
        if !evidence.active {
            return false;
        }
        let word = if raw.bytes().any(|b| b.is_ascii_uppercase()) {
            std::borrow::Cow::Owned(raw.to_ascii_lowercase())
        } else {
            std::borrow::Cow::Borrowed(raw)
        };
        if let Some(value) = evidence.facts.get(word.as_ref()) {
            *value
        } else {
            evidence.missing.insert(word.into_owned());
            false
        }
    })
}
pub struct Probe {
    pub romaji: Option<std::sync::Arc<crate::romaji::CustomTable>>,
    pub words: Vec<String>,
    pub before: String,
    pub after: String,
    pub level: String,
}
impl Probe {
    pub fn evaluate(&self) {
        let _romaji = crate::romaji::Scope::new(self.romaji.clone());
        let dictionary = crate::dictionary::Dictionary::bundled();
        for word in &self.words {
            crate::protection::transform(word, |part| {
                for final_input in [false, true] {
                    dictionary.composition(
                        part,
                        final_input,
                        &self.before,
                        &self.after,
                        &self.level,
                    );
                }
                dictionary.mixed_context(part, true, false);
                dictionary.known_english(part);
                String::new()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_evidence_is_scoped_and_unknown_queries_are_collected() {
        assert!(!is_word("puretuma"));
        {
            let scope = Scope::new(HashMap::from([("puretuma".into(), true)]));
            assert!(is_word("Puretuma"));
            assert!(!is_word("unknown"));
            assert_eq!(scope.missing(), vec!["unknown"]);
            {
                let _inner = Scope::new(HashMap::new());
                assert!(!is_word("puretuma"));
            }
            assert!(is_word("puretuma"));
        }
        assert!(!is_word("puretuma"));
    }
    #[test]
    fn host_spelling_is_weaker_than_japanese_and_manual_policy() {
        let _scope = Scope::new(HashMap::from([
            ("puretuma".into(), true),
            ("kore".into(), true),
            ("mala".into(), true),
        ]));
        let d = crate::dictionary::Dictionary::bundled();
        assert_eq!(
            d.composition("puretuma", true, "we use ", "", "Conservative"),
            "puretuma"
        );
        assert_eq!(
            d.composition("puretuma", true, "", "", "Conservative"),
            "ぷれつま"
        );
        assert_eq!(
            d.composition("puretuma", true, "we use ", "", "Manual"),
            "ぷれつま"
        );
        assert_eq!(d.composition("kore", true, "", "", "Balanced"), "これ");
        assert_eq!(d.composition("mala", true, "", "", "Conservative"), "まぁ");
    }
    #[test]
    fn unwind_restores_parent_evidence() {
        let _parent = Scope::new(HashMap::from([("word".into(), true)]));
        let _ = std::panic::catch_unwind(|| {
            let _scope = Scope::new(HashMap::new());
            panic!("probe");
        });
        assert!(is_word("word"));
    }
}
