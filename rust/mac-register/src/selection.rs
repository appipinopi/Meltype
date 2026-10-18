// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
const TARGET: &str = "io.github.yksr-melt.inputmethod.Meltype.Japanese";
pub trait SelectionSystem {
    type List;
    fn list(&mut self) -> Option<Self::List>;
    fn count(&self, list: &Self::List) -> usize;
    fn id(&self, list: &Self::List, index: usize) -> Option<String>;
    fn select(&mut self, list: &Self::List, index: usize) -> i32;
    fn current_id(&mut self) -> Option<String>;
    fn wait(&mut self);
    fn stdout(&mut self, text: &str);
    fn stderr(&mut self, text: &str);
}
/// Original ten attempts, 300 ms waits, success only after reading back the
/// requested source. Duplicate matching entries remain eligible within a pass.
pub fn select(system: &mut impl SelectionSystem) -> i32 {
    for _ in 0..10 {
        if let Some(sources) = system.list() {
            for index in 0..system.count(&sources) {
                if system.id(&sources, index).as_deref() == Some(TARGET)
                    && system.select(&sources, index) == 0
                    && system.current_id().as_deref() == Some(TARGET)
                {
                    system.stdout("Meltype selected.\n");
                    return 0;
                }
            }
        }
        system.wait();
    }
    system.stderr("Unable to select Meltype.\n");
    1
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};
    type Trace = Rc<RefCell<Vec<String>>>;
    struct List {
        trace: Trace,
        ids: Vec<Option<String>>,
    }
    impl Drop for List {
        fn drop(&mut self) {
            self.trace.borrow_mut().push("release list".into())
        }
    }
    struct Fake {
        trace: Trace,
        ids: Vec<Option<String>>,
        statuses: VecDeque<i32>,
        current: VecDeque<Option<String>>,
        out: String,
        err: String,
    }
    impl Default for Fake {
        fn default() -> Self {
            Self {
                trace: Trace::default(),
                ids: vec![None, Some("other".into()), Some(TARGET.into())],
                statuses: VecDeque::new(),
                current: VecDeque::new(),
                out: String::new(),
                err: String::new(),
            }
        }
    }
    impl SelectionSystem for Fake {
        type List = List;
        fn list(&mut self) -> Option<List> {
            self.trace.borrow_mut().push("list enabled".into());
            Some(List {
                trace: self.trace.clone(),
                ids: self.ids.clone(),
            })
        }
        fn count(&self, list: &List) -> usize {
            list.ids.len()
        }
        fn id(&self, list: &List, index: usize) -> Option<String> {
            list.ids[index].clone()
        }
        fn select(&mut self, _: &List, index: usize) -> i32 {
            self.trace.borrow_mut().push(format!("select {index}"));
            self.statuses.pop_front().unwrap_or(-50)
        }
        fn current_id(&mut self) -> Option<String> {
            self.trace.borrow_mut().push("copy current".into());
            self.current.pop_front().flatten()
        }
        fn wait(&mut self) {
            self.trace.borrow_mut().push("sleep 300ms".into());
        }
        fn stdout(&mut self, text: &str) {
            self.out.push_str(text);
        }
        fn stderr(&mut self, text: &str) {
            self.err.push_str(text);
        }
    }
    #[test]
    fn verifies_selection_and_keeps_duplicate_sources_eligible() {
        let mut api = Fake {
            ids: vec![Some(TARGET.into()), Some(TARGET.into())],
            statuses: VecDeque::from([-50, 0]),
            current: VecDeque::from([Some(TARGET.into())]),
            ..Fake::default()
        };
        assert_eq!(select(&mut api), 0);
        assert_eq!(
            *api.trace.borrow(),
            [
                "list enabled",
                "select 0",
                "select 1",
                "copy current",
                "release list"
            ]
        );
        assert_eq!(api.out, "Meltype selected.\n");
        assert!(api.err.is_empty());
    }
    #[test]
    fn current_source_mismatch_or_missing_retries_then_succeeds() {
        let mut api = Fake {
            statuses: VecDeque::from([0, 0, 0]),
            current: VecDeque::from([None, Some("other".into()), Some(TARGET.into())]),
            ..Fake::default()
        };
        assert_eq!(select(&mut api), 0);
        assert_eq!(
            api.trace
                .borrow()
                .iter()
                .filter(|v| *v == "sleep 300ms")
                .count(),
            2
        );
        assert_eq!(
            api.trace
                .borrow()
                .iter()
                .filter(|v| *v == "release list")
                .count(),
            3
        );
        assert_eq!(api.out, "Meltype selected.\n");
        assert!(api.err.is_empty());
    }
    #[test]
    fn ten_failed_attempts_preserve_error_waits_and_release() {
        for ids in [
            vec![],
            vec![Some("other".into())],
            vec![Some(TARGET.into())],
        ] {
            let mut api = Fake {
                ids,
                ..Fake::default()
            };
            assert_eq!(select(&mut api), 1);
            assert_eq!(
                api.trace
                    .borrow()
                    .iter()
                    .filter(|v| *v == "list enabled")
                    .count(),
                10
            );
            assert_eq!(
                api.trace
                    .borrow()
                    .iter()
                    .filter(|v| *v == "sleep 300ms")
                    .count(),
                10
            );
            assert_eq!(
                api.trace
                    .borrow()
                    .iter()
                    .filter(|v| *v == "release list")
                    .count(),
                10
            );
            assert!(api.out.is_empty());
            assert_eq!(api.err, "Unable to select Meltype.\n");
        }
    }
}
