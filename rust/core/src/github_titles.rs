// SPDX-License-Identifier: GPL-3.0-or-later
//! Session-memory Issue title cache. The transport receives only repository/number.
use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
pub fn normalize(text: &str) -> String {
    let mut value = text.trim().trim_end_matches('/');
    for prefix in ["https://", "http://", "www.", "github.com/"] {
        if value
            .get(..prefix.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(prefix))
        {
            value = &value[prefix.len()..];
        }
    }
    if value.len() >= 4
        && value
            .get(value.len() - 4..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(".git"))
    {
        value = &value[..value.len() - 4];
    }
    let parts: Vec<_> = value.split('/').collect();
    if parts.len() >= 2 {
        format!("{}/{}", parts[0].trim(), parts[1].trim())
    } else {
        value.into()
    }
}
pub fn valid(repository: &str) -> bool {
    let Some((owner, name)) = repository.split_once('/') else {
        return false;
    };
    !owner.is_empty()
        && !name.is_empty()
        && repository
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/._-".contains(&c))
        && !name.contains('/')
}
struct ResultState {
    complete: bool,
    title: Option<String>,
}
struct Entry {
    started: Instant,
    waited: bool,
    result: Arc<(Mutex<ResultState>, Condvar)>,
}
#[derive(Default)]
pub struct Cache {
    entries: Mutex<HashMap<(String, u32), Entry>>,
}
impl Cache {
    pub fn lookup(
        &self,
        repository: &str,
        number: u32,
        fetch: impl FnOnce(String, u32) -> Option<String> + Send + 'static,
    ) -> Option<String> {
        self.lookup_with(
            repository,
            number,
            Duration::from_millis(1500),
            Duration::from_secs(60),
            fetch,
        )
    }
    fn lookup_with(
        &self,
        repository: &str,
        number: u32,
        timeout: Duration,
        retry: Duration,
        fetch: impl FnOnce(String, u32) -> Option<String> + Send + 'static,
    ) -> Option<String> {
        let repository = normalize(repository);
        if !valid(&repository) || number == 0 {
            return None;
        }
        let key = (repository.clone(), number);
        let (result, wait) = {
            let mut entries = self.entries.lock().ok()?;
            if let Some(entry) = entries.get(&key) {
                let state = entry.result.0.lock().ok()?;
                if state.complete {
                    let title = state.title.clone();
                    let retry_now = title.is_none() && entry.started.elapsed() >= retry;
                    drop(state);
                    if retry_now {
                        entries.remove(&key);
                    }
                    return title;
                }
            }
            let entry = entries.entry(key).or_insert_with(|| {
                let result = Arc::new((
                    Mutex::new(ResultState {
                        complete: false,
                        title: None,
                    }),
                    Condvar::new(),
                ));
                let worker = result.clone();
                std::thread::spawn(move || {
                    let title = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        fetch(repository, number)
                    }))
                    .ok()
                    .flatten();
                    if let Ok(mut state) = worker.0.lock() {
                        state.title = title;
                        state.complete = true;
                        worker.1.notify_all();
                    }
                });
                Entry {
                    started: Instant::now(),
                    waited: false,
                    result,
                }
            });
            let wait = !entry.waited;
            entry.waited = true;
            (entry.result.clone(), wait)
        };
        let state = result.0.lock().ok()?;
        let state = if wait {
            result
                .1
                .wait_timeout_while(state, timeout, |s| !s.complete)
                .ok()?
                .0
        } else {
            state
        };
        state.title.clone()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caches_success_and_invalid_repository_never_calls_transport() {
        let cache = Cache::default();
        assert_eq!(
            cache.lookup(" https://github.com/owner/repo.git/ ", 4, |repo, n| {
                assert_eq!(repo, "owner/repo");
                assert_eq!(n, 4);
                Some("title".into())
            }),
            Some("title".into())
        );
        assert_eq!(
            cache.lookup("owner/repo", 4, |_, _| panic!("cached")),
            Some("title".into())
        );
        assert!(cache
            .lookup("owner/repo?token=x", 4, |_, _| panic!("invalid"))
            .is_none());
    }
    #[test]
    fn timed_out_lookup_waits_only_once_then_uses_completed_reply() {
        let cache = Cache::default();
        let (tx, rx) = std::sync::mpsc::channel();
        assert!(cache
            .lookup_with("a/b", 1, Duration::ZERO, Duration::ZERO, move |_, _| rx
                .recv()
                .ok())
            .is_none());
        assert!(cache
            .lookup_with(
                "a/b",
                1,
                Duration::from_secs(10),
                Duration::ZERO,
                |_, _| panic!("already pending")
            )
            .is_none());
        tx.send("done".to_owned()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if cache
                .lookup_with("a/b", 1, Duration::ZERO, Duration::ZERO, |_, _| {
                    panic!("cached")
                })
                .is_some()
            {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
}
