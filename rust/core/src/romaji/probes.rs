// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded parsing reuse owned by one session, never by the global dictionary.
use super::{analyze, custom_table, hiragana, Analysis, CustomTable};
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    marker::PhantomData,
    rc::Rc,
    sync::{Arc, Mutex},
};

const CAPACITY: usize = 4096;

pub(crate) struct Probe {
    pub reading: String,
    pub strict: Analysis,
}

#[derive(Default)]
struct Entries {
    table: Option<Arc<CustomTable>>,
    values: HashMap<String, Arc<Probe>>,
    order: VecDeque<String>,
    rendered: VecDeque<Rendered>,
}

struct Rendered {
    raw: String,
    final_input: bool,
    before: String,
    after: String,
    level: String,
    value: String,
}

#[derive(Default)]
pub(crate) struct Cache(Mutex<Entries>);
thread_local! {
    static ACTIVE: RefCell<Option<Arc<Cache>>> = const { RefCell::new(None) };
}
impl Cache {
    pub fn current() -> Option<Arc<Self>> {
        let cache = ACTIVE.with(|s| s.borrow().clone())?;
        let table = custom_table();
        {
            let mut entries = cache.0.lock().unwrap_or_else(|e| e.into_inner());
            let same = match (&entries.table, &table) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            };
            if !same {
                entries.values.clear();
                entries.order.clear();
                entries.rendered.clear();
                entries.table = table;
            }
        }
        Some(cache)
    }
    pub fn get(&self, raw: &str) -> Arc<Probe> {
        {
            let entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(probe) = entries.values.get(raw) {
                return probe.clone();
            }
        }
        let probe = Arc::new(Probe {
            reading: hiragana(raw, true),
            strict: analyze(raw, false),
        });
        // The span search only asks for <=40 ASCII bytes. Avoid retaining
        // unexpectedly large caller input if this helper is reused later.
        if raw.len() <= 40 {
            let mut entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
            if entries.values.len() == CAPACITY {
                if let Some(oldest) = entries.order.pop_front() {
                    entries.values.remove(&oldest);
                }
            }
            entries.order.push_back(raw.to_owned());
            entries.values.insert(raw.to_owned(), probe.clone());
        }
        probe
    }
    pub fn clear(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Entries::default();
    }
    pub fn composition(
        &self,
        raw: &str,
        final_input: bool,
        before: &str,
        after: &str,
        level: &str,
        compute: impl FnOnce() -> String,
    ) -> String {
        // Host spelling scopes also collect missing queries. Do not bypass
        // those side effects, or retain text from unbounded document context.
        if crate::spelling::active()
            || crate::language_memory::active()
            || raw.len() + before.len() + after.len() > 8192
        {
            return compute();
        }
        {
            let entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(row) = entries.rendered.iter().find(|r| {
                r.raw == raw
                    && r.final_input == final_input
                    && r.before == before
                    && r.after == after
                    && r.level == level
            }) {
                return row.value.clone();
            }
        }
        let value = compute();
        let mut entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if entries.rendered.len() == 4 {
            entries.rendered.pop_front();
        }
        entries.rendered.push_back(Rendered {
            raw: raw.to_owned(),
            final_input,
            before: before.to_owned(),
            after: after.to_owned(),
            level: level.to_owned(),
            value: value.clone(),
        });
        value
    }
}

pub(crate) struct Scope {
    previous: Option<Arc<Cache>>,
    _thread: PhantomData<Rc<()>>,
}
impl Scope {
    pub fn new(cache: Arc<Cache>) -> Self {
        Self {
            previous: ACTIVE.with(|s| s.replace(Some(cache))),
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
    fn cache_is_bounded_and_nested_custom_tables_are_isolated() {
        let cache = Arc::new(Cache::default());
        let _scope = Scope::new(cache.clone());
        assert_eq!(Cache::current().unwrap().get("kk").reading, "っk");
        let a = Arc::new(CustomTable::parse("kk\tきん\n"));
        let b = Arc::new(CustomTable::parse("kk\tかい\n"));
        {
            let _a = super::super::Scope::new(Some(a));
            assert_eq!(Cache::current().unwrap().get("kk").reading, "きん");
            {
                let _b = super::super::Scope::new(Some(b));
                assert_eq!(Cache::current().unwrap().get("kk").reading, "かい");
            }
            assert_eq!(Cache::current().unwrap().get("kk").reading, "きん");
        }
        assert_eq!(Cache::current().unwrap().get("kk").reading, "っk");
        for i in 0..CAPACITY + 10 {
            cache.get(&i.to_string());
        }
        let entries = cache.0.lock().unwrap();
        assert_eq!(entries.values.len(), CAPACITY);
        assert_eq!(entries.order.len(), CAPACITY);
        drop(entries);
        cache.clear();
        assert!(cache.0.lock().unwrap().values.is_empty());
    }
    #[test]
    fn rendered_results_follow_context_level_commit_and_host_spelling() {
        let dictionary = crate::dictionary::Dictionary::bundled();
        let mut expected = vec![];
        for raw in ["puretuma", "ai", "googledekensaku", "gakusein", "cameraa"] {
            for level in ["Balanced", "Manual", "Conservative", "Aggressive"] {
                for (before, after) in [("", ""), ("we use ", "today"), ("これは", "です")] {
                    for final_input in [false, true] {
                        expected.push((
                            raw,
                            final_input,
                            before,
                            after,
                            level,
                            dictionary.composition(raw, final_input, before, after, level),
                        ));
                    }
                }
            }
        }
        let host_expected: Vec<_> = [true, false, true]
            .into_iter()
            .map(|known| {
                let host = crate::spelling::Scope::new(HashMap::from([("puretuma".into(), known)]));
                let value = dictionary.composition("puretuma", true, "we use ", "", "Conservative");
                dictionary.composition("unknownword", false, "", "", "Balanced");
                (known, value, host.missing())
            })
            .collect();
        let cache = Arc::new(Cache::default());
        let _scope = Scope::new(cache.clone());
        for (raw, final_input, before, after, level, expected) in expected {
            for _ in 0..2 {
                assert_eq!(
                    dictionary.composition(raw, final_input, before, after, level),
                    expected
                );
            }
        }
        // Active spelling evidence must also run its missing-query collection
        // on a repeated render, and a replacement host scope may change facts.
        for (known, expected, missing) in host_expected {
            let host = crate::spelling::Scope::new(HashMap::from([("puretuma".into(), known)]));
            let rendered = dictionary.composition("puretuma", true, "we use ", "", "Conservative");
            assert_eq!(rendered, expected);
            dictionary.composition("unknownword", false, "", "", "Balanced");
            assert!(!missing.is_empty());
            assert_eq!(host.missing(), missing);
        }
        let table = Arc::new(CustomTable::parse("kk\tきん\n"));
        {
            let _custom = super::super::Scope::new(Some(table));
            assert_eq!(dictionary.composition("kk", true, "", "", "Manual"), "きん");
        }
        assert_eq!(dictionary.composition("kk", true, "", "", "Manual"), "っk");
        cache.clear();
        assert!(cache.0.lock().unwrap().rendered.is_empty());
    }
}
