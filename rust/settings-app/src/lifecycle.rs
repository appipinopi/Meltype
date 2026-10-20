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
}
#[cfg(any(windows, test))]
fn rust_release(body: &serde_json::Value) -> Option<String> {
    body.as_array()?.iter().find_map(|release| {
        if release["draft"] == true {
            return None;
        }
        let tag = release["tag_name"].as_str()?;
        let rust_windows = release["assets"].as_array()?.iter().any(|asset| {
            let name = asset["name"].as_str().unwrap_or("").to_ascii_lowercase();
            name.contains("rust") && name.contains("windows") && name.ends_with(".zip")
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
        std::thread::spawn(move || {
            #[cfg(windows)]
            let message = match meltype_core::windows_http::get_api_json(
                "/repos/appipinopi/Meltype/releases?per_page=20",
                None,
            ) {
                Some(body) => rust_release(&body).map_or_else(
                    || "最近の配布一覧にRust Windows版のZIPはありません。".into(),
                    |tag| {
                        format!("Rust Windows版の配布: {tag}。配布一覧で内容を確認してください。")
                    },
                ),
                None => "配布一覧を取得できませんでした。ブラウザーから確認できます。".into(),
            };
            #[cfg(not(windows))]
            let message = "このOSではブラウザーから配布一覧を確認してください。".to_owned();
            let _ = sender.send(message);
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
        let mut acknowledge = false;
        if self.welcome {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            egui::Window::new("Meltype Rust へようこそ").collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label("Windows・共通Coreの移植プレビューです。");
                ui.label("1. 設定と入力テストで、入力方式や候補を確認してください。");
                ui.label("2. IMEの導入は配布フォルダーの Manage-Windows.ps1 で行います。");
                ui.label("3. Windowsの言語設定からMeltype IMEを選択してください。");
                ui.label("ZIPを展開・起動しただけではIMEは登録されません。");
                ui.label("登録できない場合のキーボード方式への自動切替は、移植対応中です。");
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
                ui.label("Rust版の配布物を選び、SHA256SUMS.jsonを確認してから更新してください。");
                ui.label("更新: Manage-Windows.ps1 -Action Update -Package <新しい配布フォルダー>");
                ui.label("削除: Manage-Windows.ps1 -Action Uninstall（利用者データは保持）");
                ui.label("自動確認は起動時にAutoUpdateの設定に従います。自動ダウンロード・置換は対応中です。");
            });
        }
        if check_updates {
            self.check_updates();
        }
        acknowledge
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn updater_selects_only_rust_windows_packages_and_skips_drafts() {
        let releases = json!([
            {"tag_name":"v1.3.1","assets":[{"name":"Meltype-Windows.zip"}]},
            {"tag_name":"rust-draft","draft":true,"assets":[{"name":"Meltype-Rust-Windows.zip"}]},
            {"tag_name":"rust-linux","assets":[{"name":"Meltype-Rust-Linux.zip"}]},
            {"tag_name":"rust-preview","assets":[{"name":"Meltype-Rust-Windows-x64.zip"}]}
        ]);
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
