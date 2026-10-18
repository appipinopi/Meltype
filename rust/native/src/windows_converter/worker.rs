// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Per-worker engine lifetime and per-application HybridConverter policy.
use super::{
    candidates::Candidates,
    system::{Apartment, System},
};
use crate::{Callbacks, CALLBACKS};
use meltype_core::mozc::{Mozc, Segment};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_char,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Hybrid,
    Mozc,
    System,
}
impl Mode {
    fn parse(mode: &str) -> Self {
        match mode {
            "Mozc" => Self::Mozc,
            "System" => Self::System,
            _ => Self::Hybrid,
        }
    }
}
struct Backend {
    mode: Mode,
    system: System,
    extras: Candidates,
    helper: Option<PathBuf>,
    profile: PathBuf,
    mozc: Option<Mozc>,
    starting: Option<Receiver<std::io::Result<Mozc>>>,
    failures: u32,
    conversions: HashMap<(String, String), Option<Vec<Segment>>>,
    candidates: HashMap<String, Vec<String>>,
}
impl Backend {
    fn available(&self) -> bool {
        self.failures < 3 && self.helper.as_ref().is_some_and(|p| p.is_file())
    }
    fn warm_up(&mut self) {
        let completed = self.starting.as_ref().and_then(|r| match r.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err(std::io::Error::other("Mozc startup ended")))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        });
        if let Some(result) = completed {
            self.starting = None;
            match result {
                Ok(backend) => self.mozc = Some(backend),
                Err(_) => self.failures += 1,
            }
        }
        if self.mode != Mode::System
            && self.available()
            && self.mozc.is_none()
            && self.starting.is_none()
        {
            let helper = self.helper.clone().unwrap();
            let profile = self.profile.clone();
            let (sender, receiver) = mpsc::sync_channel(1);
            self.starting = Some(receiver);
            std::thread::spawn(move || {
                let _ = sender.send(Mozc::start(&helper, Some(&profile)));
            });
        }
    }
    fn remember_candidates(&mut self, segments: &[Segment], bounded: bool) {
        for segment in segments {
            if bounded && self.candidates.len() > 512 {
                self.candidates.clear();
            }
            self.candidates
                .insert(segment.reading.clone(), segment.candidates.clone());
        }
    }
    fn mozc_request(&mut self, reading: &str, context: &str) -> Option<Vec<Segment>> {
        if self.mode == Mode::System || !self.available() || reading.is_empty() {
            return None;
        }
        let clean = |s: &str| s.replace(['\t', '\r', '\n'], " ");
        let key = (clean(context), clean(reading));
        if let Some(known) = self.conversions.get(&key).cloned() {
            if let Some(ref segments) = known {
                self.remember_candidates(segments, false);
            }
            return known;
        }
        let response = self.mozc.as_mut()?.query(reading, context);
        match response {
            Ok(segments) => {
                if let Some(ref segments) = segments {
                    self.remember_candidates(segments, true);
                }
                if self.conversions.len() > 512 {
                    self.conversions.clear();
                }
                self.conversions.insert(key, segments.clone());
                segments
            }
            Err(_) => {
                self.mozc = None;
                self.failures += 1;
                self.conversions.clear();
                self.candidates.clear();
                None
            }
        }
    }
    fn use_system(&self) -> bool {
        self.mode != Mode::Mozc || !self.available()
    }
    fn convert(&mut self, reading: &str) -> Option<String> {
        if let Some(segments) = self.mozc_request(reading, "") {
            return Some(
                segments
                    .iter()
                    .map(|s| s.candidates.first().unwrap_or(&s.reading).as_str())
                    .collect(),
            );
        }
        self.use_system()
            .then(|| {
                self.system
                    .convert(&reading.encode_utf16().collect::<Vec<_>>())
            })
            .flatten()
            .map(|text| String::from_utf16_lossy(&text))
    }
    fn clauses(&mut self, reading: &str, context: &str) -> Option<Vec<(String, String)>> {
        if let Some(segments) = self.mozc_request(reading, context) {
            return Some(
                segments
                    .into_iter()
                    .map(|s| {
                        let text = s
                            .candidates
                            .first()
                            .cloned()
                            .unwrap_or_else(|| s.reading.clone());
                        (s.reading, text)
                    })
                    .collect(),
            );
        }
        if !self.use_system() {
            return None;
        }
        self.system
            .clauses(
                &reading.encode_utf16().collect::<Vec<_>>(),
                &context.encode_utf16().collect::<Vec<_>>(),
            )
            .map(|segments| {
                segments
                    .into_iter()
                    .map(|s| {
                        (
                            String::from_utf16_lossy(&s.reading),
                            String::from_utf16_lossy(&s.text),
                        )
                    })
                    .collect()
            })
    }
    fn candidates(&mut self, reading: &str) -> Vec<String> {
        let mut result = Vec::new();
        if self.mode != Mode::System && self.available() {
            if let Some(known) = self.candidates.get(reading) {
                result = known.clone();
            } else if let Some(segments) = self.mozc_request(reading, "") {
                result = if segments.len() == 1 {
                    segments[0].candidates.clone()
                } else {
                    vec![segments
                        .iter()
                        .map(|s| s.candidates.first().unwrap_or(&s.reading).as_str())
                        .collect()]
                };
            }
        }
        if self.mode != Mode::Mozc {
            for candidate in self.extras.get(reading) {
                if !result.contains(&candidate) {
                    result.push(candidate);
                }
            }
        }
        result
    }
}
thread_local! { static BACKEND: RefCell<Option<Backend>> = const { RefCell::new(None) }; }
fn with_backend<T>(f: impl FnOnce(&mut Backend) -> T) -> Option<T> {
    BACKEND
        .try_with(|state| state.try_borrow_mut().ok()?.as_mut().map(f))
        .ok()
        .flatten()
}
unsafe extern "C" fn convert(reading: *const c_char) -> *mut c_char {
    let value = crate::read(reading).and_then(|s| with_backend(|b| b.convert(&s)).flatten());
    value.map_or(std::ptr::null_mut(), |s| crate::allocate(&s))
}
unsafe extern "C" fn clauses(reading: *const c_char, context: *const c_char) -> *mut c_char {
    let context = crate::read(context).unwrap_or_default();
    let value =
        crate::read(reading).and_then(|s| with_backend(|b| b.clauses(&s, &context)).flatten());
    value.map_or(std::ptr::null_mut(), |segments| {
        crate::allocate(
            &segments
                .into_iter()
                .map(|(r, t)| format!("{r}\t{t}"))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })
}
unsafe extern "C" fn candidates(reading: *const c_char) -> *mut c_char {
    let value = crate::read(reading).and_then(|s| with_backend(|b| b.candidates(&s)));
    value.map_or(std::ptr::null_mut(), |values| {
        crate::allocate(&values.join("\n"))
    })
}

/// Pins COM to the worker's original STA thread and restores callbacks on drop.
pub struct WorkerConverters {
    previous: Callbacks,
    _apartment: Apartment,
}
impl Drop for WorkerConverters {
    fn drop(&mut self) {
        BACKEND.with(|state| {
            state.borrow_mut().take();
        });
        if let Ok(mut callbacks) = CALLBACKS.lock() {
            *callbacks = self.previous;
        }
    }
}
pub fn install() -> Option<WorkerConverters> {
    // Private SDK hosts exercise document mechanics without an installed engine.
    if std::env::var_os("MELTYPE_TEST_TIP_NO_CONVERTERS").is_some_and(|v| v == "1") {
        return None;
    }
    if !BACKEND
        .try_with(|state| state.try_borrow().ok().is_some_and(|s| s.is_none()))
        .ok()?
    {
        return None;
    }
    let apartment = Apartment::enter()?;
    let helper = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("mozc/meltype_mozc_helper.exe")));
    let mut callbacks = CALLBACKS.lock().ok()?;
    let previous = *callbacks;
    BACKEND.with(|state| {
        *state.borrow_mut() = Some(Backend {
            mode: Mode::Hybrid,
            system: System::default(),
            extras: Candidates::default(),
            helper,
            profile: meltype_core::storage::Store::default_directory().join("mozc"),
            mozc: None,
            starting: None,
            failures: 0,
            conversions: HashMap::new(),
            candidates: HashMap::new(),
        })
    });
    *callbacks = Callbacks {
        clauses: Some(clauses),
        candidates: Some(candidates),
        convert: Some(convert),
    };
    Some(WorkerConverters {
        previous,
        _apartment: apartment,
    })
}
pub(crate) struct Scope(Option<Mode>);
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(previous) = self.0 {
            with_backend(|b| b.mode = previous);
        }
    }
}
pub(crate) fn scope(mode: &str, enabled: bool) -> Scope {
    Scope(if enabled {
        with_backend(|backend| {
            let previous = backend.mode;
            backend.mode = Mode::parse(mode);
            backend.warm_up();
            previous
        })
    } else {
        None
    })
}
