// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! The legacy hook InputSession state machine. This is separate from TSF's
//! direct-input lifetime. Commands only return replay/learning actions; they
//! never inject keys, invoke host callbacks or write learning data.
//!
//! `&mut self` serializes transitions in Rust. The native Session handle already
//! runs under its registry mutex; a host dispatches returned actions afterward.
use crate::{
    detection::{self, Result as Decision, Verdict},
    settings::Settings,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyEvent {
    pub vk: i32,
    #[serde(default)]
    pub scan: i32,
    #[serde(default)]
    pub extended: bool,
    #[serde(default)]
    pub is_up: bool,
    #[serde(default)]
    pub injected: bool,
    pub time: i64,
}
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum State {
    #[default]
    Idle,
    Collecting,
    Flushing,
    Committed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    JapaneseAccepted,
    JapaneseRejected,
    StayedAccepted,
    StayedRejected,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub session_id: u64,
    pub letters: String,
    pub verdict: Verdict,
    pub user_corrected: bool,
    pub outcome: Outcome,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flush {
    pub session_id: u64,
    pub result: Decision,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub session_id: u64,
    pub state: State,
    pub decision: Option<Decision>,
    pub consumed: Option<bool>,
    pub events: Option<Vec<KeyEvent>>,
    pub flushes: Vec<Flush>,
    pub ended: Vec<Summary>,
}
fn allowed() -> bool {
    true
}
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Key {
        #[serde(flatten)]
        event: KeyEvent,
        #[serde(default, rename = "modifierDown")]
        modifier_down: bool,
        #[serde(default = "allowed")]
        allowed: bool,
        #[serde(default)]
        reason: String,
    },
    Timer {
        time: i64,
    },
    Context {
        time: i64,
    },
    Take {
        #[serde(default, rename = "sessionId")]
        session_id: Option<u64>,
    },
    Abort,
    Snapshot,
}
#[derive(Default)]
pub struct InputSession {
    model: crate::prefix_learning::Model,
    state: State,
    session_id: u64,
    pending: Vec<KeyEvent>,
    letters: String,
    keys: Vec<i32>,
    first_key: i64,
    last_key: i64,
    decision_time: i64,
    decision: Option<Decision>,
    learnable: bool,
    corrected: bool,
    end_after_flush: bool,
    ended_by_space: bool,
    previous_english_space: bool,
}
fn boundary(vk: i32) -> bool {
    matches!(vk, 0x0d | 0x20 | 0x1b | 0x09)
}
fn modifier(vk: i32) -> bool {
    matches!(vk,0x10..=0x12|0xa0..=0xa5|0x5b|0x5c)
}
fn ime_toggle(vk: i32) -> bool {
    matches!(
        vk,
        0x15 | 0x16 | 0x19 | 0x1a | 0x1c | 0x1d | 0xf0 | 0xf2 | 0xf3 | 0xf4
    )
}
fn text_key(vk: i32, settings: &Settings) -> bool {
    (0x41..=0x5a).contains(&vk)
        || settings.text("InputStyle") != "Romaji" && crate::kana::key(vk, false).is_some()
}
// Avoid overflow for malformed/out-of-order host clocks, preserving signed
// comparisons at the timeout boundaries used by the original state machine.
fn elapsed(now: i64, last: i64) -> i64 {
    now.saturating_sub(last)
}
impl InputSession {
    pub fn set_model(&mut self, model: crate::prefix_learning::Model) {
        self.model = model;
    }
    pub fn execute(&mut self, command: Command, settings: &Settings) -> Output {
        let mut out = Output {
            session_id: self.session_id,
            state: self.state,
            decision: None,
            consumed: None,
            events: None,
            flushes: vec![],
            ended: vec![],
        };
        match command {
            Command::Key {
                event,
                modifier_down,
                allowed,
                reason,
            } => {
                out.consumed =
                    Some(self.on_key(event, settings, modifier_down, allowed, &reason, &mut out));
            }
            Command::Timer { time } => {
                if self.state == State::Collecting
                    && (elapsed(time, self.last_key) >= settings.integer("IdleFlushMs")
                        || elapsed(time, self.first_key) >= settings.integer("MaxHoldMs"))
                {
                    let mut result = self.evaluate(settings, true);
                    if matches!(result.verdict, Verdict::Undecided | Verdict::Unknown) {
                        result.verdict = Verdict::Unknown;
                        result.summary =
                            if elapsed(time, self.last_key) >= settings.integer("IdleFlushMs") {
                                format!(
                                    "判断できないまま{}ms 入力がなかった",
                                    settings.integer("IdleFlushMs")
                                )
                            } else {
                                format!(
                                    "判断できないまま最大保留時間 {}ms に達した",
                                    settings.integer("MaxHoldMs")
                                )
                            };
                    }
                    self.begin_flush(result, time, &mut out);
                } else if self.state == State::Committed
                    && elapsed(time, self.last_key) > settings.integer("SessionIdleMs")
                {
                    self.end(false, &mut out);
                }
            }
            Command::Context { time } => {
                self.previous_english_space = false;
                match self.state {
                    State::Collecting => {
                        self.end_after_flush = true;
                        let mut result = self.evaluate(settings, true);
                        result.verdict = Verdict::Unknown;
                        result.summary = "フォーカスが変わった".into();
                        self.begin_flush(result, time, &mut out);
                    }
                    State::Flushing => self.end_after_flush = true,
                    State::Committed => self.end(false, &mut out),
                    State::Idle => {}
                }
            }
            Command::Take { session_id } => {
                // A delayed worker may optionally identify its flush request.
                // It must not drain a newer session after Abort/focus changes.
                if self.state == State::Flushing
                    && session_id.is_none_or(|id| id == self.session_id)
                {
                    if !self.pending.is_empty() {
                        out.events = Some(std::mem::take(&mut self.pending));
                    } else if self.end_after_flush {
                        self.end(self.ended_by_space, &mut out);
                    } else {
                        self.state = State::Committed;
                    }
                }
            }
            Command::Abort => {
                out.events = Some(std::mem::take(&mut self.pending));
                self.reset_fields();
                self.state = State::Idle;
            }
            Command::Snapshot => {}
        }
        out.state = self.state;
        out.session_id = self.session_id;
        out.decision = self.decision.clone();
        out
    }
    fn on_key(
        &mut self,
        event: KeyEvent,
        settings: &Settings,
        modifier_down: bool,
        allowed: bool,
        reason: &str,
        out: &mut Output,
    ) -> bool {
        match self.state {
            State::Flushing => {
                if !event.is_up {
                    self.last_key = event.time;
                    self.mark_boundary(event.vk);
                    if ime_toggle(event.vk) {
                        self.corrected = true;
                    }
                }
                self.pending.push(event);
                true
            }
            State::Collecting => {
                self.pending.push(event.clone());
                if event.is_up {
                    return true;
                }
                self.last_key = event.time;
                if !event.injected && text_key(event.vk, settings) && !modifier_down {
                    self.append(event.vk);
                    let result = self.evaluate(settings, false);
                    if result.verdict != Verdict::Undecided {
                        self.begin_flush(result, event.time, out);
                    }
                } else {
                    self.mark_boundary(event.vk);
                    if ime_toggle(event.vk) {
                        self.corrected = true;
                    }
                    let mut result = self.evaluate(settings, true);
                    result.verdict = Verdict::Unknown;
                    result.summary = format!("キー {:02X} で判定を打ち切り", event.vk);
                    self.begin_flush(result, event.time, out);
                }
                true
            }
            State::Committed => {
                if event.is_up {
                    return false;
                }
                if elapsed(event.time, self.last_key) > settings.integer("SessionIdleMs") {
                    self.end(false, out);
                    return self.on_idle_with_effects(
                        event,
                        settings,
                        modifier_down,
                        allowed,
                        reason,
                        out,
                    );
                }
                self.last_key = event.time;
                if ime_toggle(event.vk)
                    && elapsed(event.time, self.decision_time)
                        <= settings.integer("FeedbackWindowMs")
                {
                    self.corrected = true;
                }
                if boundary(event.vk) {
                    self.end(event.vk == 0x20, out);
                }
                false
            }
            State::Idle => {
                self.on_idle_with_effects(event, settings, modifier_down, allowed, reason, out)
            }
        }
    }
    // The idle path may request a flush immediately (e.g. kana input).
    fn on_idle_with_effects(
        &mut self,
        event: KeyEvent,
        settings: &Settings,
        modifier_down: bool,
        allowed: bool,
        reason: &str,
        out: &mut Output,
    ) -> bool {
        let consumed = self.on_idle(event.clone(), settings, modifier_down, allowed, reason);
        if self.state == State::Flushing {
            out.flushes.push(Flush {
                session_id: self.session_id,
                result: self.decision.clone().expect("decision"),
            });
        }
        consumed
    }
    fn on_idle(
        &mut self,
        event: KeyEvent,
        settings: &Settings,
        modifier_down: bool,
        allowed: bool,
        reason: &str,
    ) -> bool {
        if event.is_up || event.injected {
            return false;
        }
        self.last_key = event.time;
        if !text_key(event.vk, settings) {
            if !modifier(event.vk) {
                self.previous_english_space = false;
            }
            return false;
        }
        self.session_id += 1;
        self.first_key = event.time;
        self.letters.clear();
        self.keys.clear();
        self.decision = None;
        self.learnable = true;
        self.corrected = false;
        self.end_after_flush = false;
        self.ended_by_space = false;
        let skip = if modifier_down {
            Some("修飾キーと同時押し (ショートカットか大文字)")
        } else if settings.boolean("ContinueEnglishAfterSpace") && self.previous_english_space {
            Some("英語入力の続き")
        } else if !allowed {
            Some(reason)
        } else {
            None
        };
        if let Some(reason) = skip {
            self.state = State::Committed;
            self.learnable = false;
            self.decision = Some(Decision {
                verdict: Verdict::Unknown,
                text: String::new(),
                japanese: 0,
                english: 0,
                contributions: vec![],
                summary: reason.into(),
            });
            self.decision_time = self.last_key;
            return false;
        }
        self.append(event.vk);
        let result = self.evaluate(settings, false);
        if matches!(result.verdict, Verdict::English | Verdict::Unknown) {
            self.state = State::Committed;
            self.decision = Some(result);
            self.decision_time = event.time;
            self.learnable = false;
            return false;
        }
        self.state = if result.verdict == Verdict::Undecided {
            State::Collecting
        } else {
            State::Flushing
        };
        if self.state == State::Flushing {
            self.decision_time = event.time;
            self.decision = Some(result);
        }
        self.pending.push(event);
        true
    }
    fn append(&mut self, vk: i32) {
        self.letters.push(if (0x41..=0x5a).contains(&vk) {
            (b'a' + (vk - 0x41) as u8) as char
        } else {
            '#'
        });
        self.keys.push(vk);
    }
    fn evaluate(&self, settings: &Settings, final_input: bool) -> Decision {
        detection::evaluate_with_model(
            &self.letters,
            &self.keys,
            final_input,
            settings,
            Some(&self.model),
        )
    }
    fn begin_flush(&mut self, result: Decision, time: i64, out: &mut Output) {
        self.state = State::Flushing;
        self.decision_time = time;
        self.decision = Some(result.clone());
        out.flushes.push(Flush {
            session_id: self.session_id,
            result,
        });
    }
    fn mark_boundary(&mut self, vk: i32) {
        if boundary(vk) && !self.end_after_flush {
            self.end_after_flush = true;
            self.ended_by_space = vk == 0x20;
        }
    }
    fn end(&mut self, space: bool, out: &mut Output) {
        if let Some(decision) = &self.decision {
            if self.learnable && !self.letters.is_empty() {
                let outcome = match (decision.verdict, self.corrected) {
                    (Verdict::Japanese, false) => Outcome::JapaneseAccepted,
                    (Verdict::Japanese, true) => Outcome::JapaneseRejected,
                    (_, false) => Outcome::StayedAccepted,
                    (_, true) => Outcome::StayedRejected,
                };
                out.ended.push(Summary {
                    session_id: self.session_id,
                    letters: self.letters.clone(),
                    verdict: decision.verdict,
                    user_corrected: self.corrected,
                    outcome,
                });
            }
        }
        self.previous_english_space = space
            && self
                .decision
                .as_ref()
                .is_some_and(|d| d.verdict == Verdict::English);
        self.reset_fields();
        self.state = State::Idle;
    }
    fn reset_fields(&mut self) {
        self.pending.clear();
        self.letters.clear();
        self.keys.clear();
        self.learnable = false;
        self.corrected = false;
        self.end_after_flush = false;
        self.ended_by_space = false;
    }
}
