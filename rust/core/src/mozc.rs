// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Bounded UTF-8 line transport for upstream's optional Mozc helper.
use crate::session::Clause;
use std::{
    io::{self, BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

pub struct Mozc {
    child: Child,
    input: ChildStdin,
    output: Receiver<io::Result<String>>,
    ready: bool,
    learned_since_save: u32,
}
fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LearningClause {
    pub reading: String,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub reading: String,
    pub candidates: Vec<String>,
}
impl Mozc {
    pub fn start(helper: &Path, profile: Option<&Path>) -> io::Result<Self> {
        let mut command = Command::new(helper);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if helper.is_absolute() {
            if let Some(parent) = helper.parent() {
                command.current_dir(parent);
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        if let Some(profile) = profile {
            std::fs::create_dir_all(profile)?;
            command.arg(if helper.is_absolute() && profile.is_relative() {
                std::env::current_dir()?.join(profile)
            } else {
                profile.to_path_buf()
            });
        }
        #[cfg(windows)]
        let mut child = windows_launch::spawn(&mut command)?;
        #[cfg(not(windows))]
        let mut child = command.spawn()?;
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, output) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut bytes = Vec::new();
                let result =
                    std::io::Read::take(&mut reader, 1024 * 1024 + 1).read_until(b'\n', &mut bytes);
                let line = match result {
                    Ok(0) => break,
                    Ok(_) if bytes.len() > 1024 * 1024 => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Mozc response exceeds 1 MiB",
                    )),
                    Ok(_) => String::from_utf8(bytes)
                        .map(|s| s.trim_end_matches(['\r', '\n']).to_owned())
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
                    Err(e) => Err(e),
                };
                let failed = line.is_err();
                if sender.send(line).is_err() || failed {
                    break;
                }
            }
        });
        let mut backend = Self {
            child,
            input,
            output,
            ready: false,
            learned_since_save: 0,
        };
        if backend.receive(Duration::from_secs(10))? != "READY" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Mozc greeting",
            ));
        }
        backend.ready = true;
        Ok(backend)
    }
    fn receive(&mut self, timeout: Duration) -> io::Result<String> {
        match self.output.recv_timeout(timeout) {
            Ok(Ok(line)) => Ok(line),
            result => {
                self.abort();
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("Mozc unavailable: {result:?}"),
                ))
            }
        }
    }
    pub fn clauses(&mut self, reading: &str, context: &str) -> io::Result<Vec<Clause>> {
        self.query(reading, context)?
            .map(|segments| {
                segments
                    .into_iter()
                    .map(|s| Clause::new(s.reading, s.candidates))
                    .collect()
            })
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty Mozc result"))
    }
    /// Preserve raw candidate order for HybridConverter. An empty successful
    /// response is not an engine failure and must not trigger System fallback
    /// while a Mozc-only engine remains available.
    pub fn query(&mut self, reading: &str, context: &str) -> io::Result<Option<Vec<Segment>>> {
        // Preserve the existing conversion protocol's separator hardening.
        // Learning deliberately uses US/RS to encode its clause records.
        let clean_query = |text: &str| clean(text).replace(['\x1e', '\x1f'], " ");
        let response = self.request(&format!(
            "C\t{}\t{}",
            clean_query(context),
            clean_query(reading)
        ))?;
        if response.is_empty() {
            return Ok(None);
        }
        parse_segments(reading, &response)
            .inspect_err(|_| self.abort())
            .map(Some)
    }
    /// Same L/S protocol and successful-learning save cadence as the original
    /// MozcConverter. The caller serializes this with conversion requests and
    /// clears its candidate/conversion caches when scheduling learning.
    pub fn learn(&mut self, context: &str, clauses: &[LearningClause]) -> io::Result<bool> {
        if clauses.is_empty() {
            return Ok(false);
        }
        let records = clauses
            .iter()
            .map(|c| {
                format!(
                    "{}\x1f{}",
                    clean(&c.reading).replace('\x1f', " "),
                    clean(&c.text).replace('\x1f', " ")
                )
            })
            .collect::<Vec<_>>()
            .join("\x1e");
        let accepted = self.request(&format!("L\t{}\t{records}", clean(context)))? == "OK";
        if accepted {
            self.learned_since_save += 1;
            if self.learned_since_save >= 5 {
                self.learned_since_save = 0;
                self.request("S")?;
            }
        }
        Ok(accepted)
    }
    fn request(&mut self, line: &str) -> io::Result<String> {
        if let Err(error) = writeln!(self.input, "{line}").and_then(|_| self.input.flush()) {
            self.abort();
            return Err(error);
        }
        self.receive(Duration::from_millis(1500))
    }
    fn abort(&mut self) {
        self.ready = false;
        let _ = self.child.kill();
    }
}
#[cfg(windows)]
mod windows_launch {
    use super::*;
    use std::sync::Mutex;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetErrorMode() -> u32;
        fn SetErrorMode(mode: u32) -> u32;
    }
    static LAUNCH: Mutex<()> = Mutex::new(());
    struct Restore(u32);
    impl Drop for Restore {
        fn drop(&mut self) {
            unsafe {
                SetErrorMode(self.0);
            }
        }
    }
    pub(super) fn spawn(command: &mut Command) -> io::Result<Child> {
        let _launch = LAUNCH.lock().unwrap_or_else(|error| error.into_inner());
        let previous = unsafe { GetErrorMode() };
        unsafe {
            SetErrorMode(previous | 0x0001 | 0x8000);
        }
        let _restore = Restore(previous);
        command.spawn()
    }
}
impl Drop for Mozc {
    fn drop(&mut self) {
        // The helper saves its user data after Q/EOF. Killing a healthy
        // process skips that final save, unlike the original Dispose.
        if self.ready
            && self.child.try_wait().ok() == Some(None)
            && writeln!(self.input, "Q")
                .and_then(|_| self.input.flush())
                .is_ok()
        {
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline {
                match self.child.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                    Err(_) => break,
                }
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn parse_segments(reading: &str, response: &str) -> io::Result<Vec<Segment>> {
    let clauses: Vec<Vec<&str>> = response
        .split('\x1e')
        .map(|s| s.split('\x1f').collect())
        .collect();
    if clauses
        .iter()
        .any(|c| c.len() < 2 || c[0].is_empty() || c[1..].iter().all(|s| s.is_empty()))
        || clauses.iter().map(|c| c[0]).collect::<String>() != reading
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Mozc reading mismatch",
        ));
    }
    Ok(clauses
        .into_iter()
        .map(|c| {
            let mut candidates = Vec::new();
            for candidate in &c[1..] {
                if !candidate.is_empty() && !candidates.iter().any(|s| s == candidate) {
                    candidates.push((*candidate).to_owned());
                }
            }
            Segment {
                reading: c[0].to_owned(),
                candidates,
            }
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn parse_clauses(reading: &str, response: &str) -> io::Result<Vec<Clause>> {
        parse_segments(reading, response).map(|segments| {
            segments
                .into_iter()
                .map(|s| Clause::new(s.reading, s.candidates))
                .collect()
        })
    }
    #[test]
    fn raw_candidates_do_not_inject_a_reading_or_duplicate_backend_values() {
        assert_eq!(
            parse_segments("はし", "はし\x1f橋\x1f橋\x1f\x1f箸").unwrap(),
            vec![Segment {
                reading: "はし".into(),
                candidates: vec!["橋".into(), "箸".into()]
            }]
        );
    }
    #[test]
    fn protocol_validation() {
        assert_eq!(
            parse_clauses("きょうは", "きょう\x1f今日\x1f京\x1eは\x1fは").unwrap(),
            [
                Clause::new("きょう".into(), vec!["今日".into(), "京".into()]),
                Clause::new("は".into(), vec!["は".into()])
            ]
        );
        assert_eq!(
            parse_clauses("はし", "はし\x1f橋\x1f箸").unwrap(),
            [Clause::new("はし".into(), vec!["橋".into(), "箸".into()])]
        );
        assert!(parse_clauses("はし", "かみ\x1f紙").is_err());
        assert!(parse_clauses("はし", "").is_err());
    }
}
