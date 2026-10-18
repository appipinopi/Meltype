// SPDX-License-Identifier: GPL-3.0-or-later
// ParseCustomTable/MatchCustom from the upstream RomajiDetector.
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    marker::PhantomData,
    rc::Rc,
    sync::Arc,
};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CustomTable {
    entries: BTreeMap<String, String>,
    partials: BTreeSet<String>,
    max_length: usize,
}
impl CustomTable {
    pub fn entries(&self) -> &BTreeMap<String, String> {
        &self.entries
    }
    pub fn parse(text: &str) -> Self {
        let mut entries = BTreeMap::new();
        for line in text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            let mut fields = line.split(['\t', ' ', '　']).filter(|s| !s.is_empty());
            let (Some(spelling), Some(kana)) = (fields.next(), fields.next()) else {
                continue;
            };
            let spelling = spelling.to_ascii_lowercase();
            if !spelling.bytes().all(|c| c.is_ascii_lowercase())
                || kana.bytes().any(|c| c.is_ascii_lowercase())
            {
                continue;
            }
            entries.insert(spelling, kana.to_owned());
        }
        let mut partials = BTreeSet::new();
        let mut max_length = 0;
        for spelling in entries.keys() {
            max_length = max_length.max(spelling.len());
            for length in 1..spelling.len() {
                partials.insert(spelling[..length].to_owned());
            }
        }
        Self {
            entries,
            partials,
            max_length,
        }
    }
    pub fn load(directory: &std::path::Path) -> std::io::Result<Option<Arc<Self>>> {
        let bytes = match std::fs::read(directory.join("romaji.txt")) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let (text, _) = crate::user_dictionary::decode(&bytes)?;
        let table = Self::parse(&text);
        Ok((!table.entries.is_empty()).then(|| Arc::new(table)))
    }
    pub(crate) fn matches<'a>(&'a self, rest: &str) -> Option<(usize, &'a str)> {
        for length in (1..=self.max_length.min(rest.len())).rev() {
            let Some(piece) = rest.get(..length) else {
                continue;
            };
            if length == rest.len()
                && length < self.max_length
                && self.partials.contains(piece)
                && !self.entries.contains_key(piece)
            {
                return None;
            }
            if let Some(kana) = self.entries.get(piece) {
                return Some((length, kana));
            }
        }
        None
    }
    pub(crate) fn partial(&self, rest: &str) -> bool {
        self.partials.contains(rest)
    }
}

thread_local! { static ACTIVE:RefCell<Option<Arc<CustomTable>>> = const { RefCell::new(None) }; }
pub(crate) fn current() -> Option<Arc<CustomTable>> {
    ACTIVE.with(|table| table.borrow().clone())
}
/// Each synchronous session operation scopes its own table. Nested/reentrant
/// calls and unwinding restore the previous session; no process-wide override.
pub(crate) struct Scope {
    previous: Option<Arc<CustomTable>>,
    _thread: PhantomData<Rc<()>>,
}
impl Scope {
    pub fn new(table: Option<Arc<CustomTable>>) -> Self {
        Self {
            previous: ACTIVE.with(|s| s.replace(table)),
            _thread: PhantomData,
        }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|s| s.replace(self.previous.take()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_scopes_restore_builtins_and_unwind_to_the_previous_table() {
        let original = crate::romaji::hiragana("kk", true);
        let a = Arc::new(CustomTable::parse("kk\tきん\n"));
        let b = Arc::new(CustomTable::parse("kk\tかい\n"));
        {
            let _outer = Scope::new(Some(a));
            assert_eq!(crate::romaji::hiragana("kk", true), "きん");
            {
                let _plain = Scope::new(None);
                assert_eq!(crate::romaji::hiragana("kk", true), original);
            }
            let result = std::panic::catch_unwind(|| {
                let _nested = Scope::new(Some(b));
                assert_eq!(crate::romaji::hiragana("kk", true), "かい");
                panic!("test unwind");
            });
            assert!(result.is_err());
            assert_eq!(crate::romaji::hiragana("kk", true), "きん");
        }
        assert_eq!(crate::romaji::hiragana("kk", true), original);
    }
}
