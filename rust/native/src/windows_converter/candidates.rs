// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Same Japanese TextConversionGenerator and bounded wait as WinRtCandidates.
use std::time::Duration;
use windows::{
    core::{Interface, HSTRING},
    Data::Text::TextConversionGenerator,
    Foundation::{AsyncStatus, IAsyncInfo},
    Win32::System::SystemInformation::GetTickCount64,
};

#[derive(Default)]
pub(super) struct Candidates {
    generator: Option<TextConversionGenerator>,
    unavailable: bool,
    pub(super) timed_out: bool,
    request_deadline: Option<u64>,
}
impl Candidates {
    pub(super) fn begin_request(&mut self) {
        // All clauses share one optional-candidate budget. A separate 500ms
        // wait for every clause can exceed the TSF request's 800ms deadline.
        self.request_deadline = Some(unsafe { GetTickCount64() }.saturating_add(100));
    }
    pub(super) fn get(&mut self, reading: &str) -> Vec<String> {
        self.timed_out = false;
        if reading.is_empty() || self.unavailable {
            return Vec::new();
        }
        if self
            .request_deadline
            .is_some_and(|deadline| unsafe { GetTickCount64() } >= deadline)
        {
            self.timed_out = true;
            return Vec::new();
        }
        if self.generator.is_none() {
            match TextConversionGenerator::Create(&HSTRING::from("ja")) {
                Ok(generator) => {
                    // Match the original activation sequence, including its
                    // language-data probe before the first async request.
                    let _ = generator.LanguageAvailableButNotInstalled();
                    self.generator = Some(generator);
                }
                Err(_) => {
                    self.unavailable = true;
                    return Vec::new();
                }
            }
        }
        let result = (|| {
            let operation = self
                .generator
                .as_ref()?
                .GetCandidatesWithMaxCountAsync(&HSTRING::from(reading), 20)
                .ok()?;
            // The original queries IAsyncInfo once, before starting its
            // 500-ms budget. Avoid another QueryInterface on every poll.
            let info: IAsyncInfo = operation.cast().ok()?;
            // Environment.TickCount64 in the original uses this same Windows
            // clock; QPC/Instant would give a different boundary/resolution.
            let deadline = self
                .request_deadline
                .unwrap_or_else(|| unsafe { GetTickCount64() }.saturating_add(500));
            loop {
                match info.Status().ok()? {
                    AsyncStatus::Completed => break,
                    AsyncStatus::Started => {}
                    _ => return None,
                }
                if unsafe { GetTickCount64() } > deadline {
                    self.timed_out = true;
                    let _ = info.Cancel();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let values = operation.GetResults().ok()?;
            let mut result = Vec::new();
            for i in 0..values.Size().ok()?.min(100) {
                if let Ok(text) = values.GetAt(i) {
                    let text = text.to_string_lossy();
                    if !text.is_empty() && !result.contains(&text) {
                        result.push(text);
                    }
                }
            }
            Some(result)
        })();
        result.unwrap_or_default()
    }
}
