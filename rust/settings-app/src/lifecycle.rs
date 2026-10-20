// SPDX-License-Identifier: GPL-3.0-or-later
//! Product entry points; opening a browser never sends learned text or diagnostics.
use eframe::egui;
use meltype_core::{settings::Settings, storage::Store};
pub const RELEASES: &str = "https://github.com/appipinopi/Meltype/releases";
pub const REPORT: &str = "https://github.com/appipinopi/Meltype/issues/new";
#[derive(Default)]
pub struct Lifecycle {
    pub welcome: bool,
    pub about: bool,
    update: Option<std::sync::mpsc::Receiver<String>>,
    update_status: String,
    auto_check_pending: bool,
    #[cfg(windows)]
    auto_download: bool,
}
#[cfg(any(windows, test))]
fn rust_release(body: &serde_json::Value) -> Option<String> {
    body.as_array()?.iter().find_map(|release| {
        if release["draft"] == true || release["prerelease"] == true {
            return None;
        }
        let tag = release["tag_name"].as_str()?;
        let rust_windows = release["assets"].as_array()?.iter().any(|asset| {
            let name = asset["name"].as_str().unwrap_or("").to_ascii_lowercase();
            let digest = asset["digest"].as_str().unwrap_or("");
            name.starts_with("meltype-rust-windows")
                && name.ends_with(".zip")
                && digest
                    .strip_prefix("sha256:")
                    .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        });
        rust_windows.then(|| tag.to_owned())
    })
}
impl Lifecycle {
    pub fn new(settings: &Settings) -> Self {
        Self {
            welcome: !settings.boolean("WelcomeShown")
                || settings.text("Mode") == "Tsf" && !settings.boolean("TsfIntroduced"),
            auto_check_pending: settings.boolean("AutoUpdate"),
            #[cfg(windows)]
            auto_download: settings.boolean("AutoUpdate"),
            ..Default::default()
        }
    }
    fn check_updates(&mut self) {
        if self.update.is_some() {
            return;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        self.update = Some(receiver);
        self.update_status = "Rust版の配布を確認しています…".into();
        #[cfg(windows)]
        let auto_download = self.auto_download;
        std::thread::spawn(move || {
            #[cfg(windows)]
            let message = match meltype_core::windows_http::get_api_json(
                "/repos/appipinopi/Meltype/releases?per_page=20",
                None,
            ) {
                Some(body) => rust_release(&body).map_or_else(
                    || "最近の配布一覧にRust Windows版のZIPはありません。".into(),
                    |tag| {
                        if auto_download {
                            run_updater("Stage")
                        } else {
                            format!(
                                "Rust Windows版の配布: {tag}。ダウンロード・更新を実行できます。"
                            )
                        }
                    },
                ),
                None => "配布一覧を取得できませんでした。ブラウザーから確認できます。".into(),
            };
            #[cfg(not(windows))]
            let message = "このOSではブラウザーから配布一覧を確認してください。".to_owned();
            let _ = sender.send(message);
        });
    }
    #[cfg(windows)]
    fn apply_update(&mut self) {
        if self.update.is_some() {
            return;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        self.update = Some(receiver);
        self.update_status = "更新を実行しています。Windowsの管理者確認が表示されます。".into();
        std::thread::spawn(move || {
            let _ = sender.send(run_updater("Apply"));
        });
    }
    pub fn show(&mut self, ctx: &egui::Context, store: &Store) -> bool {
        if self.auto_check_pending {
            self.auto_check_pending = false;
            self.check_updates();
        }
        if let Some(receiver) = &self.update {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
            match receiver.try_recv() {
                Ok(message) => {
                    self.update_status = message;
                    self.update = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.update_status =
                        "配布確認が終了しました。ブラウザーで確認してください。".into();
                    self.update = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        let mut check_updates = false;
        #[cfg(windows)]
        let mut apply_update = false;
        let mut acknowledge = false;
        if self.welcome {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            egui::Window::new("Meltype Rust へようこそ").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label("Windows・共通Coreの移植プレビューです。");
                ui.label("1. 設定と入力テストで、入力方式や候補を確認してください。");
                ui.label("2. IMEの導入は配布フォルダーの Manage-Windows.ps1 で行います。");
                ui.label("3. Windowsの言語設定からMeltype IMEを選択してください。");
                ui.label("ZIPを展開・起動しただけではIMEは登録されません。");
                ui.label("登録なしで使う場合は Start-Portable.ps1 を実行し、Ctrl+Alt+J で入力を切り替えます。");
                ui.label(format!("データ保存先: {}", store.directory.display()));
                ui.label("元版のデータは自動で置き換えません。移行前にバックアップを保存してください。");
                ui.hyperlink_to("導入と配布の説明", "https://github.com/appipinopi/Meltype/blob/codex/rust-cross-platform/rust/WINDOWS-PACKAGE.md");
                if ui.button("設定を始める").clicked() { acknowledge = true; }
            });
        }
        if self.about {
            egui::Window::new("Meltype Rust / 報告・更新").open(&mut self.about).show(ctx, |ui| {
                ui.label("Rust移植プレビュー（機能の参照元: Meltype 1.3.0）");
                ui.label(format!("ビルド: {}", env!("CARGO_PKG_VERSION")));
                ui.hyperlink_to("不具合・要望を報告する", REPORT);
                ui.label("報告ページはブラウザーで開きます。入力内容や学習データは自動添付しません。");
                if ui.add_enabled(self.update.is_none(), egui::Button::new("更新を確認する")).clicked() { check_updates = true; }
                ui.label(&self.update_status);
                ui.hyperlink_to("Rust版の配布一覧を開く", RELEASES);
                #[cfg(windows)]
                if ui.add_enabled(self.update.is_none(), egui::Button::new("ダウンロード・更新（管理者確認あり）")).clicked() { apply_update = true; }
                ui.label("Rust版の配布物を選び、SHA256SUMS.jsonを確認してから更新してください。");
                ui.label("更新: Manage-Windows.ps1 -Action Update -Package <新しい配布フォルダー>");
                ui.label("削除: Manage-Windows.ps1 -Action Uninstall（利用者データは保持）");
                ui.label("AutoUpdateがONなら起動時に公開版を確認し、検査済みZIPを自動取得します。インストールには管理者確認が必要です。");
            });
        }
        if check_updates {
            self.check_updates();
        }
        #[cfg(windows)]
        if apply_update {
            self.apply_update();
        }
        acknowledge
    }
}
#[cfg(windows)]
fn run_updater(action: &str) -> String {
    use std::os::windows::{ffi::OsStringExt, process::CommandExt};
    let result = (|| -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let script = exe
            .parent()
            .ok_or("配布フォルダーがありません")?
            .join("Update-Windows.ps1");
        if !script.is_file() {
            return Err(
                "更新スクリプトがありません。新しいWindows配布ZIPから起動してください。".into(),
            );
        }
        let mut system = vec![0u16; 32768];
        let n = unsafe {
            windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(
                system.as_mut_ptr(),
                system.len() as u32,
            )
        } as usize;
        if n == 0 || n >= system.len() {
            return Err("Windowsフォルダーを取得できません".into());
        }
        let shell = std::path::PathBuf::from(std::ffi::OsString::from_wide(&system[..n]))
            .join("WindowsPowerShell/v1.0/powershell.exe");
        if action == "Apply" {
            let log = tempfile::Builder::new()
                .prefix("Meltype-update-")
                .suffix(".log")
                .tempfile()
                .map_err(|e| e.to_string())?;
            let path = log.path().to_owned();
            let output = log.as_file().try_clone().map_err(|e| e.to_string())?;
            let errors = log.as_file().try_clone().map_err(|e| e.to_string())?;
            log.into_temp_path().keep().map_err(|e| e.to_string())?;
            std::process::Command::new(&shell)
                .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                .arg(&script)
                .args(["-Action", action])
                .creation_flags(0x08000000)
                .stdin(std::process::Stdio::null())
                .stdout(output)
                .stderr(errors)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(format!(
                "更新処理を開始しました。管理者確認後にトレイを再起動します。成功・失敗のログ: {}",
                path.display()
            ));
        }
        let output = std::process::Command::new(shell)
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(script)
            .args(["-Action", action])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!("更新処理が失敗またはキャンセルされました（終了コード {:?}）。配布フォルダーからUpdate-Windows.ps1を実行すると詳細を確認できます。", output.status.code()));
        }
        Ok(if action == "Stage" {
            "公開Rust版ZIPの取得・検査が完了しました。更新ボタンでインストールできます。"
        } else {
            "更新処理が完了しました。新しい配布フォルダーからトレイを起動してください。"
        }
        .into())
    })();
    result.unwrap_or_else(|error| error)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn updater_selects_only_rust_windows_packages_and_skips_drafts() {
        let mut releases = json!([
            {"tag_name":"v1.3.1","assets":[{"name":"Meltype-Windows.zip"}]},
            {"tag_name":"rust-draft","draft":true,"assets":[{"name":"Meltype-Rust-Windows.zip"}]},
            {"tag_name":"rust-linux","assets":[{"name":"Meltype-Rust-Linux.zip"}]},
            {"tag_name":"rust-preview","assets":[{"name":"Meltype-Rust-Windows-x64.zip"}]}
        ]);
        for release in releases.as_array_mut().unwrap() {
            for asset in release["assets"].as_array_mut().unwrap() {
                asset["digest"] = json!(format!("sha256:{}", "a".repeat(64)));
            }
        }
        assert_eq!(rust_release(&releases).as_deref(), Some("rust-preview"));
        assert!(rust_release(&json!([])).is_none());
        let settings = Settings::parse(r#"{"AutoUpdate":false,"SettingsVersion":5}"#).unwrap();
        let mut lifecycle = Lifecycle::new(&settings);
        assert!(!lifecycle.auto_check_pending);
        let directory = tempfile::tempdir().unwrap();
        let store = Store::new(directory.path());
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            lifecycle.show(ctx, &store);
        });
        assert!(
            lifecycle.update.is_none(),
            "AutoUpdate=false must not start a request"
        );
    }
}
