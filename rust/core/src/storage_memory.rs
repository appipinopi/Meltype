// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared learning model without an OS filesystem. The host persists snapshots.
#[path = "storage/state.rs"]
mod state;
pub use state::{Change, LanguageEntry, Phrase, State};

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
