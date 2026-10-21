// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Standalone installer registration helper; no IME/native-core/Swift dependency.
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
pub use native::{native_main, native_select_main};
mod selection;
pub use selection::{select, SelectionSystem};

/// Platform operations used by the original helper's synchronous protocol.
pub trait System {
    fn default_app_path(&mut self) -> String;
    fn exists(&mut self, path: &str) -> bool;
    fn register(&mut self, path: &str) -> i32;
    fn bundle_id(&mut self, path: &str) -> Option<String>;
    fn enabled(&mut self, id: &str) -> bool;
    fn wait(&mut self);
    fn restart_agent(&mut self) -> Result<(), String>;
    fn stdout(&mut self, text: &str);
    fn stderr(&mut self, text: &str);
}
/// Preserve the old helper's messages, exit code, thirty 100 ms waits and
/// best-effort agent refresh. Extra command arguments are ignored as before.
pub fn run(system: &mut impl System, path: Option<String>) -> i32 {
    let path = path.unwrap_or_else(|| system.default_app_path());
    if !system.exists(&path) {
        system.stderr(&format!("Meltype.app が見つかりません: {path}\n"));
        return 1;
    }
    let status = system.register(&path);
    if status != 0 {
        system.stderr(&format!(
            "入力ソースに登録できませんでした (OSStatus {status}): {path}\n"
        ));
        return 1;
    }
    system.stdout(&format!("入力ソースに Meltype を登録しました: {path}\n"));
    if let Some(id) = system.bundle_id(&path) {
        let mut available = false;
        for _ in 0..30 {
            if system.enabled(&id) {
                available = true;
                break;
            }
            system.wait();
        }
        if available {
            if let Err(error) = system.restart_agent() {
                system.stderr(&format!(
                    "入力メニューの表示を更新できませんでした: {error}\n"
                ));
            }
            system.stdout("入力メニューの表示を更新しました\n");
        } else {
            system.stdout("まだ入力ソースとして有効になっていません。ログアウトしてログインし直すと直ります。\n");
        }
    } else {
        system.stdout("Meltype.app の Bundle ID が読めませんでした。ログアウトしてログインし直すと直ります。\n");
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        calls: Vec<String>,
        out: String,
        err: String,
        exists: bool,
        status: i32,
        id: Option<String>,
        enabled_at: usize,
        polls: usize,
        restart_error: bool,
    }
    impl Default for Fake {
        fn default() -> Self {
            Self {
                calls: vec![],
                out: String::new(),
                err: String::new(),
                exists: true,
                status: 0,
                id: Some("fixture.bundle".into()),
                enabled_at: 0,
                polls: 0,
                restart_error: false,
            }
        }
    }
    impl System for Fake {
        fn default_app_path(&mut self) -> String {
            self.calls.push("bundle path".into());
            "default.app".into()
        }
        fn exists(&mut self, path: &str) -> bool {
            self.calls.push(format!("exists {path}"));
            self.exists
        }
        fn register(&mut self, path: &str) -> i32 {
            self.calls.push(format!("register {path}"));
            self.status
        }
        fn bundle_id(&mut self, _: &str) -> Option<String> {
            self.calls.push("bundle ID".into());
            self.id.clone()
        }
        fn enabled(&mut self, id: &str) -> bool {
            self.calls.push(format!("enabled {id}"));
            let poll = self.polls;
            self.polls += 1;
            poll >= self.enabled_at
        }
        fn wait(&mut self) {
            self.calls.push("sleep 100ms".into());
        }
        fn restart_agent(&mut self) -> Result<(), String> {
            self.calls.push("restart agent".into());
            if self.restart_error {
                Err("fixture error".into())
            } else {
                Ok(())
            }
        }
        fn stdout(&mut self, text: &str) {
            self.out.push_str(text);
        }
        fn stderr(&mut self, text: &str) {
            self.err.push_str(text);
        }
    }
    #[test]
    fn missing_and_registration_errors_stop_before_polling() {
        let mut api = Fake {
            exists: false,
            ..Fake::default()
        };
        assert_eq!(run(&mut api, Some("辞書 空間.app".into())), 1);
        assert_eq!(api.calls, ["exists 辞書 空間.app"]);
        assert_eq!(api.err, "Meltype.app が見つかりません: 辞書 空間.app\n");
        assert!(api.out.is_empty());
        let mut api = Fake {
            status: -50,
            ..Fake::default()
        };
        assert_eq!(run(&mut api, None), 1);
        assert_eq!(
            api.calls,
            ["bundle path", "exists default.app", "register default.app"]
        );
        assert_eq!(
            api.err,
            "入力ソースに登録できませんでした (OSStatus -50): default.app\n"
        );
        assert!(api.out.is_empty());
    }
    #[test]
    fn poll_boundaries_and_best_effort_refresh_preserve_original_protocol() {
        for available_at in [0, 1, 29, 30, usize::MAX] {
            for restart_error in [false, true] {
                let mut api = Fake {
                    enabled_at: available_at,
                    restart_error,
                    ..Fake::default()
                };
                assert_eq!(run(&mut api, Some("owned.app".into())), 0);
                assert_eq!(api.polls, (available_at.saturating_add(1)).min(30));
                assert_eq!(
                    api.calls.iter().filter(|c| *c == "sleep 100ms").count(),
                    available_at.min(30)
                );
                assert_eq!(
                    api.calls.iter().filter(|c| *c == "restart agent").count(),
                    usize::from(available_at < 30)
                );
                assert!(api
                    .out
                    .starts_with("入力ソースに Meltype を登録しました: owned.app\n"));
                if available_at < 30 {
                    assert!(api.out.ends_with("入力メニューの表示を更新しました\n"));
                    assert_eq!(
                        api.err,
                        if restart_error {
                            "入力メニューの表示を更新できませんでした: fixture error\n"
                        } else {
                            ""
                        }
                    );
                } else {
                    assert!(api.out.ends_with("まだ入力ソースとして有効になっていません。ログアウトしてログインし直すと直ります。\n"));
                    assert!(api.err.is_empty());
                }
            }
        }
    }
    #[test]
    fn absent_bundle_id_does_not_poll_or_refresh_and_is_success() {
        let mut api = Fake {
            id: None,
            ..Fake::default()
        };
        assert_eq!(run(&mut api, None), 0);
        assert_eq!(
            api.calls,
            [
                "bundle path",
                "exists default.app",
                "register default.app",
                "bundle ID"
            ]
        );
        assert!(api.out.ends_with("Meltype.app の Bundle ID が読めませんでした。ログアウトしてログインし直すと直ります。\n"));
        assert!(api.err.is_empty());
    }
}
