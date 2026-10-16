// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;
use meltype_core::{control, settings::Settings, storage::Store, Session};
use serde_json::{json, Value};
mod metadata;
mod rules;
#[cfg(windows)]
mod tray;
mod widgets;

fn main() -> eframe::Result {
    let resident = cfg!(windows) && std::env::args().any(|a| a == "--tray");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([740., 740.])
            .with_min_inner_size([480., 400.]),
        ..Default::default()
    };
    let mut options = options;
    options.viewport = options.viewport.with_visible(!resident);
    eframe::run_native(
        "Meltype 設定",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            let mut fonts = egui::FontDefinitions::default();
            let paths = std::env::var_os("MELTYPE_FONT")
                .map(std::path::PathBuf::from)
                .into_iter()
                .chain(
                    [
                        "C:/Windows/Fonts/YuGothM.ttc",
                        "C:/Windows/Fonts/meiryo.ttc",
                        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
                        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                    ]
                    .into_iter()
                    .map(std::path::PathBuf::from),
                );
            for path in paths {
                if let Ok(bytes) = std::fs::read(path) {
                    fonts
                        .font_data
                        .insert("japanese".into(), egui::FontData::from_owned(bytes).into());
                    fonts
                        .families
                        .entry(egui::FontFamily::Proportional)
                        .or_default()
                        .insert(0, "japanese".into());
                    cc.egui_ctx.set_fonts(fonts);
                    break;
                }
            }
            Ok(Box::new(App::new()))
        }),
    )
}
struct App {
    store: Store,
    settings: Settings,
    opened: Settings,
    discard: bool,
    closing: bool,
    help: String,
    #[cfg(windows)]
    resident: bool,
    #[cfg(windows)]
    tray: Option<tray::Tray>,
    page: usize,
    status: String,
    filter: String,
    reading: String,
    word: String,
    snippet_name: String,
    snippet_text: String,
    path: String,
    profile: String,
    profile_prompt: Option<bool>,
    confirm_remove: bool,
    test_raw: String,
    test_session: Session,
    result: Value,
    complex: BTreeEditor,
    show_all: bool,
    #[cfg(feature = "visual-test")]
    capture_frames: usize,
}
#[derive(Default)]
struct BTreeEditor(std::collections::BTreeMap<String, String>);
impl App {
    fn new() -> Self {
        let store = Store::new(Store::default_directory());
        Self::with_store(store)
    }
    fn with_store(store: Store) -> Self {
        let loaded = store.settings();
        let status = loaded
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_default();
        let settings = loaded.unwrap_or_default();
        // The input test must not write learning or settings to disk.
        let mut test_session = Session::default();
        test_session.settings = settings.clone();
        Self {
            store,
            opened: settings.clone(),
            settings,
            discard: false,
            closing: false,
            help: "項目にカーソルを合わせると説明を表示します。".into(),
            #[cfg(windows)]
            resident: std::env::args().any(|a| a == "--tray"),
            #[cfg(windows)]
            tray: None,
            page: 0,
            status,
            filter: String::new(),
            reading: String::new(),
            word: String::new(),
            snippet_name: String::new(),
            snippet_text: String::new(),
            path: String::new(),
            profile: String::new(),
            profile_prompt: None,
            confirm_remove: false,
            test_raw: String::new(),
            test_session,
            result: Value::Null,
            complex: BTreeEditor::default(),
            show_all: false,
            #[cfg(feature = "visual-test")]
            capture_frames: 0,
        }
    }
    fn action(&mut self, request: Value) {
        let operation = request["op"].as_str().unwrap_or("");
        let name = request["name"].as_str().unwrap_or("");
        let draft = match operation {
            "settings_set" => Some(self.settings.set(name, request["value"].clone())),
            "profile_add" => Some(
                self.settings
                    .add_profile(name)
                    .and_then(|()| self.settings.switch_profile(name)),
            ),
            "profile_switch" => Some(self.settings.switch_profile(name)),
            "profile_delete" => Some(self.settings.delete_profile(name)),
            "profile_rename" => Some(self.settings.rename_profile(
                self.settings.text("ActiveProfile").to_owned().as_str(),
                name,
            )),
            "profile_remove" => Some(self.settings.remove_profile(name)),
            _ => None,
        };
        if let Some(result) = draft {
            match result {
                Ok(()) => {
                    self.status = "変更は「OK」または閉じるボタンで保存します。".into();
                    self.test_session.settings = self.settings.clone();
                }
                Err(error) => self.status = error,
            }
            return;
        }
        match control::execute(&self.store, &request) {
            Ok(result) => {
                self.result = result;
                self.status = "保存しました / Saved".into();
            }
            Err(error) => self.status = error.to_string(),
        }
    }
    fn save(&mut self) -> bool {
        if self.settings.0 == self.opened.0 {
            return true;
        }
        match control::execute(
            &self.store,
            &json!({"op":"settings_merge", "opened":self.opened, "edited":self.settings}),
        ) {
            Ok(_) => {
                self.opened = self.settings.clone();
                true
            }
            Err(error) => {
                self.status = format!("保存できません: {error}");
                false
            }
        }
    }
    fn finish(&mut self, ctx: &egui::Context, discard: bool) {
        if !discard && !self.save() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            return;
        }
        if discard {
            match self.store.settings() {
                Ok(settings) => {
                    self.opened = settings.clone();
                    self.settings = settings.clone();
                    self.test_session.settings = settings;
                    self.complex.0.clear();
                }
                Err(error) => self.status = error.to_string(),
            }
        }
        #[cfg(windows)]
        if self.resident {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            self.discard = false;
            return;
        }
        self.discard = discard;
        self.closing = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    #[cfg(windows)]
    fn update_tray(&mut self, ctx: &egui::Context) {
        if !self.resident {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        let settings = match self.store.settings() {
            Ok(s) => s,
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        if self.tray.is_none() {
            match tray::Tray::new(ctx, &settings, &self.store) {
                Ok(tray) => self.tray = Some(tray),
                Err(error) => {
                    self.status = error;
                    self.resident = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    return;
                }
            }
        }
        let actions = self.tray.as_ref().unwrap().poll();
        for action in actions {
            match action {
                tray::Action::Page(page) => {
                    // Keep an open draft; otherwise start a fresh settings window.
                    if ctx.input(|i| i.viewport().minimized.unwrap_or(false))
                        || self.settings.0 == self.opened.0
                    {
                        self.settings = settings.clone();
                        self.opened = settings.clone();
                        self.test_session.settings = settings.clone();
                    }
                    self.page = page;
                    self.discard = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                tray::Action::Folder => {
                    if let Err(e) = std::fs::create_dir_all(&self.store.directory).and_then(|()| {
                        std::process::Command::new("explorer.exe")
                            .arg(&self.store.directory)
                            .spawn()
                            .map(|_| ())
                    }) {
                        self.status = e.to_string();
                    }
                }
                tray::Action::Quit => {
                    if self.save() {
                        if let Err(error) = self
                            .store
                            .change_settings(|s| s.set("Enabled", json!(false)))
                        {
                            self.status = error.to_string();
                            continue;
                        }
                        self.closing = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                action => {
                    if let Err(error) = tray::write_action(&self.store, &action) {
                        self.status = error;
                    }
                }
            }
        }
        if let Ok(settings) = self.store.settings() {
            if let Some(tray) = &mut self.tray {
                if let Err(error) = tray.sync(&settings) {
                    self.status = error;
                }
            }
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("プロファイル:");
            let active = self.settings.text("ActiveProfile").to_owned();
            let mut selected = active.clone();
            egui::ComboBox::from_id_salt("active_profile")
                .selected_text(&active)
                .show_ui(ui, |ui| {
                    if let Some(profiles) = self.settings.0["Profiles"].as_array() {
                        for profile in profiles {
                            if let Some(name) = profile["Name"].as_str() {
                                ui.selectable_value(&mut selected, name.to_owned(), name);
                            }
                        }
                    }
                });
            if selected != active {
                self.action(json!({"op":"profile_switch","name":selected}));
            }
            if ui.button("新規...").clicked() {
                self.profile.clear();
                self.profile_prompt = Some(false);
            }
            if ui.button("名前を変更...").clicked() {
                self.profile = active;
                self.profile_prompt = Some(true);
            }
            if ui
                .add_enabled(
                    self.settings.0["Profiles"]
                        .as_array()
                        .is_some_and(|p| p.len() > 1),
                    egui::Button::new("削除"),
                )
                .clicked()
            {
                self.confirm_remove = true;
            }
            if ui.button("書き出す...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("プロファイルを書き出す")
                    .add_filter("Meltype プロファイル", &["json"])
                    .set_file_name("Meltype-profile.json")
                    .save_file()
                {
                    match serde_json::to_vec_pretty(&self.settings.export_profile())
                        .map_err(std::io::Error::other)
                        .and_then(|bytes| meltype_core::storage::atomic_write(&path, &bytes))
                    {
                        Ok(()) => self.status =
                            "プロファイルを書き出しました。アプリ別設定のプロセス名も含まれます。"
                                .into(),
                        Err(error) => self.status = error.to_string(),
                    }
                }
            }
            if ui.button("読み込む...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("プロファイルを読み込む")
                    .add_filter("Meltype プロファイル", &["json"])
                    .pick_file()
                {
                    match std::fs::read_to_string(path)
                        .map_err(|e| e.to_string())
                        .and_then(|text| self.settings.import_profile(&text))
                    {
                        Ok(name) => {
                            self.status =
                                format!("「{name}」を読み込みました。「OK」で保存します。");
                            self.test_session.settings = self.settings.clone();
                        }
                        Err(error) => self.status = error,
                    }
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (key, label) in [
                ("SettingsShowHelp", "下部に説明を表示"),
                ("SettingsWheelChangesValue", "マウスホイールで値を変更"),
            ] {
                let mut enabled = self.settings.boolean(key);
                if ui.checkbox(&mut enabled, label).changed() {
                    self.action(json!({"op":"settings_set","name":key,"value":enabled}));
                }
            }
        });
        ui.checkbox(
            &mut self.show_all,
            "未移植の設定項目も表示（現在は保存のみ）",
        );
        ui.horizontal(|ui| {
            ui.label("検索 / Filter");
            ui.text_edit_singleline(&mut self.filter);
        });
        let mut fields: Vec<_> = metadata::fields()
            .iter()
            .filter_map(|f| {
                let key = f["key"].as_str()?;
                let value = self.settings.0.get(key)?;
                let filter = self.filter.to_lowercase();
                ((self.show_all || supported(key))
                    && (key.to_lowercase().contains(&filter)
                        || metadata::label(key).contains(&self.filter)
                        || metadata::description(key).contains(&self.filter)))
                .then(|| {
                    (
                        key.to_owned(),
                        value.clone(),
                        f["category"].as_str().unwrap_or("").to_owned(),
                    )
                })
            })
            .collect();
        fields.sort_by(|a, b| a.2.cmp(&b.2));
        egui::ScrollArea::vertical()
            .id_salt("settings_body")
            .show(ui, |ui| {
                let mut category = String::new();
                for (key, mut value, group) in fields {
                    if group != category {
                        ui.add_space(12.);
                        ui.heading(
                            group
                                .split_once(". ")
                                .map_or(group.as_str(), |(_, label)| label),
                        );
                        ui.separator();
                        category = group;
                    }
                    ui.push_id(&key, |ui| {
                        if key == "AppRules" || key == "AppKinds" {
                            let label = ui
                                .add_sized(
                                    [ui.available_width() * 0.55, 28.],
                                    egui::Label::new(metadata::label(&key)).wrap(),
                                )
                                .on_hover_text(metadata::description(&key));
                            if label.hovered() {
                                self.help = metadata::description(&key).into();
                            }
                            if rules::editor(ui, &key, &mut value, &self.settings) {
                                self.action(json!({"op":"settings_set","name":key,"value":value}));
                            }
                            return;
                        }
                        let row = ui.horizontal(|ui| {
                            let label_width = ui.available_width() * 0.55;
                            let label = ui
                                .allocate_ui_with_layout(
                                    egui::vec2(label_width, 28.),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.set_min_width(label_width);
                                        ui.label(metadata::label(&key))
                                            .on_hover_text(metadata::description(&key))
                                    },
                                )
                                .inner;
                            if label.hovered() {
                                self.help = metadata::description(&key).into();
                            }
                            match &mut value {
                                Value::Bool(v) => {
                                    if widgets::toggle(ui, v, metadata::label(&key))
                                        .on_hover_text(metadata::description(&key))
                                        .changed()
                                    {
                                        self.action(
                                            json!({"op":"settings_set","name":key,"value":value}),
                                        );
                                    }
                                }
                                Value::Number(n) => {
                                    let mut number = n.as_i64().unwrap_or(0);
                                    let response = ui.add(egui::DragValue::new(&mut number));
                                    let step = widgets::wheel_step(
                                        ui,
                                        &response,
                                        self.settings.boolean("SettingsWheelChangesValue"),
                                    );
                                    number = number.saturating_add(step);
                                    if response.changed() || step != 0 {
                                        self.action(
                                            json!({"op":"settings_set","name":key,"value":number}),
                                        );
                                    }
                                }
                                Value::String(text) => {
                                    if let Some(choices) = choices(&key) {
                                        let old = text.clone();
                                        let combo = egui::ComboBox::from_id_salt(&key)
                                            .selected_text(metadata::choice(&key, text))
                                            .show_ui(ui, |ui| {
                                                for choice in choices {
                                                    ui.selectable_value(
                                                        text,
                                                        (*choice).to_owned(),
                                                        metadata::choice(&key, choice),
                                                    );
                                                }
                                            });
                                        let step = widgets::wheel_step(
                                            ui,
                                            &combo.response,
                                            self.settings.boolean("SettingsWheelChangesValue"),
                                        );
                                        if step != 0 {
                                            if let Some(index) = choices
                                                .iter()
                                                .position(|choice| *choice == text.as_str())
                                            {
                                                *text = choices[(index as i64 - step)
                                                    .clamp(0, choices.len() as i64 - 1)
                                                    as usize]
                                                    .to_string();
                                            }
                                        }
                                        if *text != old {
                                            self.action(
                                            json!({"op":"settings_set","name":key,"value":text}),
                                        );
                                        }
                                    } else {
                                        let response = ui.text_edit_singleline(text);
                                        if response.changed() {
                                            self.settings.0.insert(key.clone(), json!(text));
                                        }
                                    }
                                }
                                _ => {
                                    let text =
                                        self.complex.0.entry(key.clone()).or_insert_with(|| {
                                            serde_json::to_string(&value).unwrap_or_default()
                                        });
                                    ui.text_edit_singleline(text);
                                    if ui.button("保存 JSON").clicked() {
                                        match serde_json::from_str::<Value>(text) {
                                        Ok(value) => self.action(
                                            json!({"op":"settings_set","name":key,"value":value}),
                                        ),
                                        Err(e) => self.status = e.to_string(),
                                    }
                                    }
                                }
                            }
                        });
                        if row.response.hovered() {
                            self.help = metadata::description(&key).into();
                        }
                        ui.separator();
                    });
                }
            });
    }
    fn dictionary(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("読み");
            ui.text_edit_singleline(&mut self.reading);
            ui.label("単語");
            ui.text_edit_singleline(&mut self.word);
        });
        ui.horizontal(|ui| {
            if ui.button("登録 / Add").clicked() {
                self.action(json!({"op":"dictionary_set","reading":self.reading,"word":self.word}));
            }
            if ui.button("削除 / Remove").clicked() {
                self.action(
                    json!({"op":"dictionary_remove","reading":self.reading,"word":self.word}),
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label("ファイル");
            ui.text_edit_singleline(&mut self.path);
            if ui.button("取込").clicked() {
                self.action(json!({"op":"dictionary_import","path":self.path}));
            }
            if ui.button("書出").clicked() {
                self.action(json!({"op":"dictionary_export","path":self.path}));
            }
        });
        match self.store.load() {
            Ok(state) => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (reading, words) in state.preferences.user_dictionary {
                        for word in words {
                            if ui
                                .selectable_label(false, format!("{reading}　{word}"))
                                .clicked()
                            {
                                self.reading = reading.clone();
                                self.word = word;
                            }
                        }
                    }
                });
            }
            Err(e) => {
                ui.label(e.to_string());
            }
        }
    }
    fn snippets(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("名前");
            ui.text_edit_singleline(&mut self.snippet_name);
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.snippet_text)
                .desired_rows(6)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui.button("登録 / Save").clicked() {
                self.action(
                    json!({"op":"snippet_set","name":self.snippet_name,"text":self.snippet_text}),
                );
            }
            if ui.button("削除 / Remove").clicked() {
                self.action(json!({"op":"snippet_remove","name":self.snippet_name}));
            }
        });
        if let Ok(state) = self.store.load() {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (name, text) in state.snippets {
                    if ui.selectable_label(false, &name).clicked() {
                        self.snippet_name = name;
                        self.snippet_text = text;
                    }
                }
            });
        }
    }
    fn learning(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("更新 / Refresh").clicked() {
                self.action(json!({"op":"learning_list"}));
            }
            if ui.button("直近1時間の学習を消去").clicked() {
                self.action(
                    json!({"op":"learning_forget_since","since":meltype_core::storage::now()-3600}),
                );
            }
        });
        ui.collapsing("学習データをすべて消す", |ui| {
            ui.label("辞書と定型文は残ります。");
            if ui.button("全学習を消去 / Clear all learning").clicked() {
                self.action(json!({"op":"learning_clear"}));
            }
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.monospace(serde_json::to_string_pretty(&self.result).unwrap_or_default());
        });
    }
    fn backup(&mut self, ui: &mut egui::Ui) {
        ui.label("バックアップのファイル名（絶対パス）");
        ui.text_edit_singleline(&mut self.path);
        ui.horizontal(|ui| {
            if ui.button("作成 / Create").clicked() {
                self.action(json!({"op":"backup_create","path":self.path}));
            }
            if ui.button("内容を確認 / Inspect").clicked() {
                self.action(json!({"op":"backup_inspect","path":self.path}));
            }
        });
        ui.collapsing("バックアップを復元", |ui| {
            ui.label("同名の現在のデータを上書きします。元データは .before-restore に残します。");
            if ui.button("このファイルから復元 / Restore").clicked() {
                self.action(json!({"op":"backup_restore","path":self.path}));
            }
        });
        if let Some(files) = self.result["files"].as_object() {
            for name in files.keys() {
                ui.label(name);
            }
        }
    }
    fn test(&mut self, ui: &mut egui::Ui) {
        ui.label("ローマ字入力を試す / Type romaji");
        if ui.text_edit_singleline(&mut self.test_raw).changed() {
            self.test_session.reset();
            for c in self.test_raw.chars() {
                self.test_session.key(
                    c.to_ascii_uppercase() as i32,
                    c as i32,
                    i32::from(c.is_ascii_uppercase()),
                );
            }
        }
        if !self.test_raw.is_empty() && self.test_raw.len() <= 256 {
            let keys: Vec<_> = self
                .test_raw
                .chars()
                .map(|c| c.to_ascii_uppercase() as i32)
                .collect();
            let result = meltype_core::detection::evaluate(
                &self.test_raw.to_ascii_lowercase(),
                &keys,
                true,
                &self.settings,
            );
            let language = match result.verdict {
                meltype_core::detection::Verdict::Japanese => "日本語",
                meltype_core::detection::Verdict::English => "英語",
                _ => "未確定",
            };
            ui.label(format!(
                "英数状態での判定: {language}（日本語 {} / 英語 {}）",
                result.japanese, result.english
            ));
            ui.collapsing("判定理由", |ui| {
                for contribution in result.contributions {
                    ui.label(contribution.reason);
                }
            });
        }
        ui.horizontal(|ui| {
            for (label, vk) in [
                ("変換", 32),
                ("ひらがな", 0x75),
                ("カタカナ", 0x76),
                ("半角カナ", 0x77),
                ("英字", 0x79),
            ] {
                if ui.button(label).clicked() {
                    self.test_session.key(vk, 0, 0);
                }
            }
        });
        let view = self.test_session.result(false).view;
        if let Some(view) = view {
            ui.heading(view.text);
            if let Some(meaning) = view.meaning {
                ui.label(meaning);
            }
            if let Some(suggestion) = view.suggestion {
                ui.label(format!("もしかして: {suggestion}"));
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, candidate) in view.candidates.iter().enumerate() {
                    if ui.button(candidate).clicked() {
                        let result = self.test_session.select(i);
                        self.test_raw.clear();
                        self.status = result
                            .commits
                            .iter()
                            .map(|e| e.text.as_str())
                            .collect::<String>();
                    }
                }
            });
        }
    }
}
fn supported(key: &str) -> bool {
    [
        "Enabled",
        "LiveConversion",
        "DirectModeAutoDetect",
        "JapaneseThreshold",
        "TypoEnabled",
        "MaxPendingKeys",
        "AppRules",
        "AppKinds",
        "LearningEnabled",
        "InputStyle",
        "Punctuation",
        "CorrectTypos",
        "AutoCorrectAfterCommit",
        "TranslationCandidates",
        "PredictiveCandidates",
        "TabConversion",
        "ContinueAfterConversion",
        "SeparateKeyRoles",
        "ShowCandidateMeanings",
        "ShowTypedKeys",
        "SnippetMark",
        "SpaceAroundEnglish",
        "SlashAsMiddleDot",
    ]
    .contains(&key)
}
fn choices(key: &str) -> Option<&'static [&'static str]> {
    Some(match key {
        "DetectionLevel" => &["Aggressive", "Balanced", "Conservative", "Manual"],
        "InputStyle" => &["Romaji", "Kana", "Both"],
        "Punctuation" => &[
            "Japanese",
            "FullWidthCommaPeriod",
            "FullWidthCommaKuten",
            "ToutenFullWidthPeriod",
        ],
        "Mode" => &["Keyboard", "AutoSwitch", "Tsf"],
        "ConversionEngine" => &["Hybrid", "Mozc", "System"],
        "CompositionPlacement" => &["Overlay", "BelowCaret", "AboveCaret"],
        "CompositionSize" => &["Auto", "Small", "Medium", "Large", "ExtraLarge", "Huge"],
        "CompositionTheme" => &["Dark", "Light", "System"],
        "CompositionOpacity" => &["Opaque", "Percent90", "Percent80", "Percent70"],
        _ => return None,
    })
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        #[cfg(windows)]
        self.update_tray(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.closing {
            self.finish(ctx, self.discard);
        }
        #[cfg(feature = "visual-test")]
        if let Ok(path) = std::env::var("MELTYPE_UI_CAPTURE") {
            self.capture_frames += 1;
            ctx.request_repaint();
            if self.capture_frames == 20 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    std::fs::write(format!("{path}.rgba"), rgba).expect("write capture");
                    std::fs::write(format!("{path}.json"), json!(image.size).to_string())
                        .expect("write capture size");
                    #[cfg(windows)]
                    std::fs::write(
                        format!("{path}.state.json"),
                        json!({"resident":self.resident,"trayCreated":self.tray.is_some()})
                            .to_string(),
                    )
                    .expect("write tray capture state");
                    self.closing = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Meltype 設定");
                ui.label("移植プレビュー");
            });
        });
        egui::SidePanel::left("navigation")
            .resizable(false)
            .default_width(160.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    for (i, label) in [
                        "設定",
                        "ユーザー辞書",
                        "定型文",
                        "学習",
                        "バックアップ",
                        "入力テスト",
                    ]
                    .iter()
                    .enumerate()
                    {
                        ui.selectable_value(&mut self.page, i, *label);
                    }
                });
            });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("既定値に戻す").clicked() {
                    let defaults = Settings::default();
                    for (key, value) in defaults.0 {
                        if !["Profiles", "ActiveProfile", "SettingsVersion"].contains(&key.as_str())
                        {
                            self.settings.0.insert(key, value);
                        }
                    }
                    self.test_session.settings = self.settings.clone();
                    self.status = "既定値に戻しました。「OK」で保存します。".into();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("キャンセル").clicked() {
                        self.finish(ctx, true);
                    }
                    if ui.button("OK").clicked() {
                        self.finish(ctx, false);
                    }
                });
            });
            ui.label(&self.status);
        });
        if self.page == 0 {
            if self.settings.boolean("SettingsShowHelp") {
                egui::TopBottomPanel::bottom("help")
                    .exact_height(80.)
                    .resizable(false)
                    .show(ctx, |ui| {
                        ui.label(&self.help).on_hover_text(&self.help);
                    });
            }
            egui::TopBottomPanel::bottom("detection_test")
                .exact_height(115.)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("判定テスト (IME 自動切替の判定。英字で入力):");
                    ui.text_edit_singleline(&mut self.test_raw);
                    if !self.test_raw.is_empty() && self.test_raw.len() <= 256 {
                        let keys: Vec<_> = self
                            .test_raw
                            .chars()
                            .map(|c| c.to_ascii_uppercase() as i32)
                            .collect();
                        let result = meltype_core::detection::evaluate(
                            &self.test_raw.to_ascii_lowercase(),
                            &keys,
                            true,
                            &self.settings,
                        );
                        let verdict = match result.verdict {
                            meltype_core::detection::Verdict::Japanese => "日本語",
                            meltype_core::detection::Verdict::English => "英語",
                            _ => "未確定",
                        };
                        egui::ScrollArea::vertical()
                            .id_salt("detection_reasons")
                            .show(ui, |ui| {
                                ui.label(format!(
                                    "{verdict}  日本語 {} / 英語 {}",
                                    result.japanese, result.english
                                ));
                                for contribution in result.contributions {
                                    ui.label(contribution.reason);
                                }
                            });
                    }
                });
        }
        egui::CentralPanel::default().show(ctx, |ui| match self.page {
            0 => self.settings(ui),
            1 => self.dictionary(ui),
            2 => self.snippets(ui),
            3 => self.learning(ui),
            4 => self.backup(ui),
            _ => self.test(ui),
        });
        if let Some(rename) = self.profile_prompt {
            egui::Window::new(if rename {
                "プロファイルの名前を変更"
            } else {
                "新しいプロファイル"
            })
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("名前 (例: 仕事用、趣味用、SNS 用)");
                ui.text_edit_singleline(&mut self.profile);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        let result = if rename {
                            let active = self.settings.text("ActiveProfile").to_owned();
                            self.settings.rename_profile(&active, &self.profile)
                        } else {
                            self.settings
                                .add_profile(&self.profile)
                                .and_then(|()| self.settings.switch_profile(&self.profile))
                        };
                        match result {
                            Ok(()) => {
                                self.profile_prompt = None;
                                self.test_session.settings = self.settings.clone();
                                self.status = "「OK」で保存します。".into();
                            }
                            Err(error) => self.status = error,
                        }
                    }
                    if ui.button("キャンセル").clicked() {
                        self.profile_prompt = None;
                    }
                });
            });
        }
        if self.confirm_remove {
            egui::Window::new("プロファイルを削除")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    let active = self.settings.text("ActiveProfile").to_owned();
                    ui.label(format!("プロファイル「{active}」を削除しますか？"));
                    ui.horizontal(|ui| {
                        if ui.button("削除").clicked() {
                            self.action(json!({"op":"profile_remove","name":active}));
                            self.confirm_remove = false;
                        }
                        if ui.button("キャンセル").clicked() {
                            self.confirm_remove = false;
                        }
                    });
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_edits_are_drafts_and_save_merges_only_changed_fields() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let mut app = App::with_store(store.clone());
        app.action(json!({"op":"settings_set","name":"LiveConversion","value":false}));
        assert!(!app.settings.boolean("LiveConversion"));
        assert!(store.settings().unwrap().boolean("LiveConversion"));
        assert!(!dir.path().join("config.json").exists());
        store
            .change_settings(|s| s.set("Enabled", json!(false)))
            .unwrap();
        assert!(app.save());
        let saved = store.settings().unwrap();
        assert!(!saved.boolean("LiveConversion"));
        assert!(!saved.boolean("Enabled"));
        app.action(json!({"op":"settings_set","name":"LiveConversion","value":true}));
        drop(app); // Closing without save (Cancel) must not flush a draft.
        assert!(!store.settings().unwrap().boolean("LiveConversion"));
    }
    #[test]
    fn dictionary_actions_do_not_replace_unsaved_settings_and_test_input_does_not_learn_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let mut app = App::with_store(store.clone());
        app.action(json!({"op":"settings_set","name":"LiveConversion","value":false}));
        app.action(json!({"op":"dictionary_list"}));
        assert!(!app.settings.boolean("LiveConversion"));
        app.action(json!({"op":"profile_add","name":"仕事"}));
        assert_eq!(app.settings.text("ActiveProfile"), "仕事");
        assert_eq!(store.settings().unwrap().text("ActiveProfile"), "標準");
        for c in "hashi".chars() {
            app.test_session.key(0, c as i32, 0);
        }
        app.test_session.key(32, 32, 0);
        app.test_session.select(0);
        assert!(!dir.path().join("state.json").exists());
    }
    #[cfg(windows)]
    #[test]
    fn cancel_in_resident_mode_discards_the_draft_before_hiding() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let mut app = App::with_store(store.clone());
        app.resident = true;
        app.action(json!({"op":"settings_set","name":"LiveConversion","value":false}));
        app.action(json!({"op":"profile_add","name":"未保存"}));
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.finish(ctx, true));
        assert_eq!(app.settings.0, store.settings().unwrap().0);
        assert_eq!(app.opened.0, app.settings.0);
        assert!(!app.closing);
        assert!(!dir.path().join("config.json").exists());
    }
}
