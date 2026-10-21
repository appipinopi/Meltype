// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-operation learned language evidence for each candidate span.
use std::{cell::RefCell, collections::BTreeMap, marker::PhantomData, rc::Rc};
thread_local! { static ACTIVE:RefCell<BTreeMap<String,bool>>=const {RefCell::new(BTreeMap::new())}; }
pub(crate) fn get(raw: &str) -> Option<bool> {
    ACTIVE.with(|s| s.borrow().get(raw).copied())
}
pub(crate) fn active() -> bool {
    ACTIVE.with(|s| !s.borrow().is_empty())
}
pub(crate) fn relevant(raw: &str) -> bool {
    ACTIVE.with(|s| {
        s.borrow()
            .keys()
            .any(|word| !word.is_empty() && raw.to_ascii_lowercase().contains(word))
    })
}
pub(crate) struct Scope {
    previous: BTreeMap<String, bool>,
    _thread: PhantomData<Rc<()>>,
}
impl Scope {
    pub fn new(values: &BTreeMap<String, bool>) -> Self {
        Self {
            previous: ACTIVE.with(|s| s.replace(values.clone())),
            _thread: PhantomData,
        }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        ACTIVE.with(|s| s.replace(std::mem::take(&mut self.previous)));
    }
}
