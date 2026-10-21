// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Resolve OS evidence before taking the session lock. No callback runs under
//! the provider/cache/session mutexes. Cached positives and negatives expire.
use super::*;
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
#[derive(Default)]
struct State {
    callback: Option<WordCallback>,
    epoch: u64,
    cache: HashMap<String, bool>,
    expires: Option<Instant>,
    inflight: HashSet<(u64, String)>,
    checks: u64,
}
fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(State::default()))
}
pub fn configure(callback: Option<WordCallback>) {
    let mut s = state().lock().unwrap_or_else(|e| e.into_inner());
    s.callback = callback;
    s.epoch = s.epoch.wrapping_add(1);
    s.cache.clear();
    s.expires = None;
}
#[no_mangle]
pub extern "C" fn meltype_word_check_count() -> u64 {
    state().lock().unwrap_or_else(|e| e.into_inner()).checks
}
fn expire(s: &mut State, now: Instant) {
    if s.expires.is_none_or(|end| now >= end) {
        s.cache.clear();
        s.expires = Some(now + Duration::from_secs(300));
    }
}
fn remember(s: &mut State, word: &str, value: bool) {
    if s.cache.len() >= 4096 {
        s.cache.clear();
    }
    s.cache.insert(word.to_owned(), value);
}
fn disabled() -> Option<meltype_core::spelling::Scope> {
    meltype_core::spelling::active().then(meltype_core::spelling::Scope::disabled)
}
unsafe fn resolve(epoch: u64, callback: WordCallback, word: &str) -> Result<bool, ()> {
    let key = (epoch, word.to_owned());
    {
        let mut s = state().lock().unwrap_or_else(|e| e.into_inner());
        if s.epoch != epoch {
            return Err(());
        }
        expire(&mut s, Instant::now());
        if let Some(value) = s.cache.get(word) {
            return Ok(*value);
        }
        if !s.inflight.insert(key.clone()) {
            return Ok(false);
        }
        s.checks = s.checks.saturating_add(1);
    }
    let text = CString::new(word).map_err(|_| ())?;
    let value = callback(text.as_ptr()) != 0;
    let mut s = state().lock().unwrap_or_else(|e| e.into_inner());
    s.inflight.remove(&key);
    if s.epoch != epoch {
        return Err(());
    }
    remember(&mut s, word, value);
    Ok(value)
}
unsafe fn prepare_inner(
    handle: usize,
    vk: i32,
    ch: i32,
    modifiers: i32,
    before: *const c_char,
    after: *const c_char,
) -> Result<Option<meltype_core::spelling::Scope>, ()> {
    let (callback, epoch) = {
        let s = state().lock().unwrap_or_else(|e| e.into_inner());
        (s.callback, s.epoch)
    };
    let Some(callback) = callback else {
        return Ok(disabled());
    };
    let before = read(before);
    let after = read(after);
    let probe = sessions().lock().ok().and_then(|all| {
        all.get(&handle)
            .and_then(|s| s.spelling_probe(vk, ch, modifiers, before.as_deref(), after.as_deref()))
    });
    let Some(probe) = probe else {
        return Ok(disabled());
    };
    let mut facts = HashMap::new();
    loop {
        let scope = meltype_core::spelling::Scope::new(facts.clone());
        probe.evaluate();
        let missing = scope.missing();
        if missing.is_empty() {
            return Ok(Some(scope));
        }
        drop(scope);
        for word in missing {
            facts.insert(word.clone(), resolve(epoch, callback, &word)?);
            if !sessions().lock().map_err(|_| ())?.contains_key(&handle) {
                return Err(());
            }
        }
    }
}

pub unsafe fn prepare(
    handle: usize,
    vk: i32,
    ch: i32,
    modifiers: i32,
    before: *const c_char,
    after: *const c_char,
) -> Result<Option<meltype_core::spelling::Scope>, ()> {
    catch_unwind(AssertUnwindSafe(|| {
        prepare_inner(handle, vk, ch, modifiers, before, after)
    }))
    .unwrap_or(Err(()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_cache_drops_old_positive_and_negative_entries() {
        let mut s = State::default();
        for number in 0..4096 {
            remember(&mut s, &number.to_string(), number % 2 == 0);
        }
        remember(&mut s, "next", true);
        assert_eq!(s.cache.len(), 1);
        assert_eq!(s.cache.get("next"), Some(&true));
    }
    #[test]
    fn expiry_clears_both_positive_and_negative_evidence() {
        let now = Instant::now();
        let mut s = State {
            expires: Some(now - Duration::from_secs(1)),
            ..State::default()
        };
        s.cache.insert("positive".into(), true);
        s.cache.insert("negative".into(), false);
        expire(&mut s, now);
        assert!(s.cache.is_empty());
        s.cache.insert("fresh".into(), false);
        expire(&mut s, now + Duration::from_secs(1));
        assert_eq!(s.cache.get("fresh"), Some(&false));
    }
}
