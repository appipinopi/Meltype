// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Bounded UTF-8 line transport for upstream's optional Mozc helper.
use std::{
    io::{self, BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

pub struct Mozc {
    child: Child,
    input: ChildStdin,
    output: Receiver<io::Result<String>>,
}
impl Mozc {
    pub fn start(helper: &Path, profile: Option<&Path>) -> io::Result<Self> {
        let mut command = Command::new(helper);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        if let Some(profile) = profile {
            std::fs::create_dir_all(profile)?;
            command.arg(profile);
        }
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
        };
        if backend.receive(Duration::from_secs(10))? != "READY" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Mozc greeting",
            ));
        }
        Ok(backend)
    }
    fn receive(&mut self, timeout: Duration) -> io::Result<String> {
        match self.output.recv_timeout(timeout) {
            Ok(Ok(line)) => Ok(line),
            result => {
                let _ = self.child.kill();
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("Mozc unavailable: {result:?}"),
                ))
            }
        }
    }
    pub fn candidates(&mut self, reading: &str, context: &str) -> io::Result<Vec<String>> {
        let clean = |s: &str| s.replace(['\t', '\n', '\r', '\x1e', '\x1f'], " ");
        writeln!(self.input, "C\t{}\t{}", clean(context), clean(reading))?;
        self.input.flush()?;
        let response = self.receive(Duration::from_millis(1500))?;
        parse_candidates(reading, &response)
    }
}
impl Drop for Mozc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn parse_candidates(reading: &str, response: &str) -> io::Result<Vec<String>> {
    let clauses: Vec<Vec<&str>> = response
        .split('\x1e')
        .map(|s| s.split('\x1f').collect())
        .collect();
    if clauses.iter().any(|c| c.len() < 2)
        || clauses.iter().map(|c| c[0]).collect::<String>() != reading
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Mozc reading mismatch",
        ));
    }
    if clauses.len() == 1 {
        Ok(clauses[0][1..]
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| (*s).to_owned())
            .collect())
    } else {
        Ok(vec![clauses.iter().map(|c| c[1]).collect()])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_validation() {
        assert_eq!(
            parse_candidates("きょうは", "きょう\x1f今日\x1f京\x1eは\x1fは").unwrap(),
            ["今日は"]
        );
        assert_eq!(
            parse_candidates("はし", "はし\x1f橋\x1f箸").unwrap(),
            ["橋", "箸"]
        );
        assert!(parse_candidates("はし", "かみ\x1f紙").is_err());
        assert!(parse_candidates("はし", "").is_err());
    }
}
