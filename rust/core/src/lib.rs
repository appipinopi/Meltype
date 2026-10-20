// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
mod candidate_sources;
mod commit_correction;
mod composition_units;
pub mod context;
pub mod control;
pub mod detection;
pub mod dictionary;
pub mod input_session;
pub mod kana;
mod language_memory;
pub mod linguistics;
pub mod math_candidates;
pub mod mozc;
mod phrase_spacing;
pub mod prefix_learning;
pub mod protection;
pub mod romaji;
pub mod session;
pub mod settings;
mod sigil;
pub mod spelling;
pub mod storage;
pub mod time_candidates;
pub mod typo;
pub mod user_dictionary;
pub use session::{Session, SessionResult};
