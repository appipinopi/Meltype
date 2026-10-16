// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;
use meltype_core::{control, settings::Settings, storage::Store, Session};
use serde_json::{json, Value};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([940., 720.]),
        ..Default::default()
    };
    eframe::run_native(
        "Meltype Rust",
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
    page: usize,
    status: String,
    filter: String,
    reading: String,
    word: String,
    snippet_name: String,
    snippet_text: String,
    path: String,
    profile: String,
    test_raw: String,
    test_session: Session,
    result: Value,
    complex: BTreeEditor,
    show_all: bool,
}
#[derive(Default)]
struct BTreeEditor(std::collections::BTreeMap<String, String>);
impl App {
    fn new() -> Self {
        let store = Store::new(Store::default_directory());
        let loaded = store.settings();
        let status = loaded
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_default();
        let settings = loaded.unwrap_or_default();
        let test_session = Session::open(store.clone()).unwrap_or_default();
        Self {
            store,
            settings,
            page: 0,
            status,
            filter: String::new(),
            reading: String::new(),
            word: String::new(),
            snippet_name: String::new(),
            snippet_text: String::new(),
            path: String::new(),
            profile: String::new(),
            test_raw: String::new(),
            test_session,
            result: Value::Null,
            complex: BTreeEditor::default(),
            show_all: false,
        }
    }
    fn action(&mut self, request: Value) {
        match control::execute(&self.store, &request) {
            Ok(result) => {
                self.result = result;
                self.status = "保存しました / Saved".into();
                if let Ok(settings) = self.store.settings() {
                    self.settings = settings;
                }
                let _ = self.test_session.reload();
            }
            Err(error) => self.status = error.to_string(),
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("プロファイル / Profile");
            ui.text_edit_singleline(&mut self.profile);
            if ui.button("追加").clicked() {
                self.action(json!({"op":"profile_add","name":self.profile}));
            }
            if ui.button("切替").clicked() {
                self.action(json!({"op":"profile_switch","name":self.profile}));
            }
            if ui.button("削除").clicked() {
                self.action(json!({"op":"profile_delete","name":self.profile}));
            }
        });
        ui.label(format!("現在: {}", self.settings.text("ActiveProfile")));
        ui.checkbox(
            &mut self.show_all,
            "未移植の設定項目も表示（現在は保存のみ）",
        );
        ui.horizontal(|ui| {
            ui.label("検索 / Filter");
            ui.text_edit_singleline(&mut self.filter);
        });
        let fields: Vec<_> = self
            .settings
            .0
            .iter()
            .filter(|(key, _)| {
                !key.eq(&"Profiles")
                    && (self.show_all || supported(key))
                    && key.to_lowercase().contains(&self.filter.to_lowercase())
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (key, mut value) in fields {
                ui.push_id(&key, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_width(650.);
                        ui.label(&key);
                        match &mut value {
                            Value::Bool(v) => {
                                if ui.checkbox(v, "").changed() {
                                    self.action(
                                        json!({"op":"settings_set","name":key,"value":value}),
                                    );
                                }
                            }
                            Value::Number(n) => {
                                let mut number = n.as_i64().unwrap_or(0);
                                if ui.add(egui::DragValue::new(&mut number)).changed() {
                                    self.action(
                                        json!({"op":"settings_set","name":key,"value":number}),
                                    );
                                }
                            }
                            Value::String(text) => {
                                if let Some(choices) = choices(&key) {
                                    let old = text.clone();
                                    egui::ComboBox::from_id_salt(&key)
                                        .selected_text(text.as_str())
                                        .show_ui(ui, |ui| {
                                            for choice in choices {
                                                ui.selectable_value(
                                                    text,
                                                    (*choice).to_owned(),
                                                    *choice,
                                                );
                                            }
                                        });
                                    if *text != old {
                                        self.action(
                                            json!({"op":"settings_set","name":key,"value":text}),
                                        );
                                    }
                                } else {
                                    let response = ui.text_edit_singleline(text);
                                    if response.lost_focus() && response.changed() {
                                        self.action(
                                            json!({"op":"settings_set","name":key,"value":text}),
                                        );
                                    } else if response.changed() {
                                        self.settings.0.insert(key.clone(), json!(text));
                                    }
                                    if ui.button("保存").clicked() {
                                        self.action(
                                            json!({"op":"settings_set","name":key,"value":text}),
                                        );
                                    }
                                }
                            }
                            _ => {
                                let text = self.complex.0.entry(key.clone()).or_insert_with(|| {
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
                self.test_session.key(0, c as i32, 0);
            }
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
        "AppRules",
        "AppKinds",
        "LearningEnabled",
        "InputStyle",
        "Punctuation",
        "CorrectTypos",
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
        "InputStyle" => &["Romaji", "Kana"],
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
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Meltype Rust");
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
            ui.label(&self.status);
            ui.small(self.store.directory.to_string_lossy());
        });
        egui::CentralPanel::default().show(ctx, |ui| match self.page {
            0 => self.settings(ui),
            1 => self.dictionary(ui),
            2 => self.snippets(ui),
            3 => self.learning(ui),
            4 => self.backup(ui),
            _ => self.test(ui),
        });
    }
}
