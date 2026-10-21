// SPDX-License-Identifier: GPL-3.0-or-later
//! Cached issue-title lookups outside input locks.
pub fn lookup(repository: &str, number: u32) -> Option<String> {
    static CACHE: std::sync::OnceLock<meltype_core::github_titles::Cache> =
        std::sync::OnceLock::new();
    CACHE
        .get_or_init(Default::default)
        .lookup(repository, number, fetch)
}
fn fetch(repository: String, number: u32) -> Option<String> {
    if !meltype_core::github_titles::valid(&repository) || number == 0 {
        return None;
    }
    let token = std::env::var("MELTYPE_GITHUB_TOKEN").ok();
    meltype_core::windows_http::get_api_json(
        &format!("/repos/{repository}/issues/{number}"),
        token.as_deref(),
    )?["title"]
        .as_str()
        .map(str::to_owned)
}
