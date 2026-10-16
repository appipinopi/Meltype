// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Windows TSF companion. Menus use the same settings store as the input DLL.
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use eframe::egui;
use meltype_core::{settings::Settings, storage::Store};
use std::sync::mpsc::{self, Receiver};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu},
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};
#[derive(Clone, Debug)]
pub enum Action {
    Toggle,
    Page(usize),
    Profile(String),
    Level(String),
    Folder,
    Quit,
}
enum Event {
    Menu(MenuId),
    Toggle,
}
pub struct Tray {
    _lock: std::fs::File,
    icon: TrayIcon,
    enabled: CheckMenuItem,
    status: MenuItem,
    levels: Vec<(String, CheckMenuItem)>,
    profiles: Submenu,
    profile_items: Vec<(String, CheckMenuItem)>,
    actions: Vec<(MenuId, Action)>,
    receiver: Receiver<Event>,
    last_enabled: bool,
}
impl Tray {
    pub fn new(ctx: &egui::Context, settings: &Settings, store: &Store) -> Result<Self, String> {
        std::fs::create_dir_all(&store.directory).map_err(|e| e.to_string())?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(store.directory.join(".tray.lock"))
            .map_err(|e| e.to_string())?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .map_err(|_| "トレイは既に起動しています。設定画面を開きました。".to_owned())?;
        let menu = Menu::new();
        let status = MenuItem::new("Meltype", false, None);
        let enabled = CheckMenuItem::new(
            "Meltype を有効にする",
            true,
            settings.boolean("Enabled"),
            None,
        );
        menu.append(&status).map_err(|e| e.to_string())?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        menu.append(&enabled).map_err(|e| e.to_string())?;
        let mut actions = vec![(enabled.id().clone(), Action::Toggle)];
        let level_menu = Submenu::new("自動判定の強さ", true);
        let mut levels = vec![];
        for (name, label) in [
            ("Aggressive", "積極的"),
            ("Balanced", "標準"),
            ("Conservative", "慎重"),
            ("Manual", "手動 (提案のみ)"),
        ] {
            let item =
                CheckMenuItem::new(label, true, settings.text("DetectionLevel") == name, None);
            level_menu.append(&item).map_err(|e| e.to_string())?;
            actions.push((item.id().clone(), Action::Level(name.into())));
            levels.push((name.into(), item));
        }
        menu.append(&level_menu).map_err(|e| e.to_string())?;
        let profiles = Submenu::new("プロファイル", true);
        menu.append(&profiles).map_err(|e| e.to_string())?;
        for (label, action) in [
            ("設定...", Action::Page(0)),
            ("ユーザー辞書...", Action::Page(1)),
            ("定型文...", Action::Page(2)),
            ("学習した語...", Action::Page(3)),
            ("バックアップ...", Action::Page(4)),
            ("判定テスト...", Action::Page(5)),
            ("データフォルダを開く", Action::Folder),
        ] {
            let item = MenuItem::new(label, true, None);
            menu.append(&item).map_err(|e| e.to_string())?;
            actions.push((item.id().clone(), action));
        }
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        let quit = MenuItem::new("終了", true, None);
        menu.append(&quit).map_err(|e| e.to_string())?;
        actions.push((quit.id().clone(), Action::Quit));
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Meltype")
            .with_icon(text_icon(settings.boolean("Enabled"))?)
            .build()
            .map_err(|e| e.to_string())?;
        let (sender, receiver) = mpsc::channel();
        let menu_sender = sender.clone();
        let menu_ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = menu_sender.send(Event::Menu(event.id));
            menu_ctx.request_repaint();
        }));
        let tray_ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                let _ = sender.send(Event::Toggle);
                tray_ctx.request_repaint();
            }
        }));
        let mut tray = Self {
            _lock: lock,
            icon,
            enabled,
            status,
            levels,
            profiles,
            profile_items: vec![],
            actions,
            receiver,
            last_enabled: settings.boolean("Enabled"),
        };
        tray.sync(settings)?;
        Ok(tray)
    }
    pub fn sync(&mut self, settings: &Settings) -> Result<(), String> {
        let enabled = settings.boolean("Enabled");
        self.enabled.set_checked(enabled);
        self.status.set_text(if enabled {
            "Meltype: 有効"
        } else {
            "Meltype: 一時停止"
        });
        if enabled != self.last_enabled {
            self.icon
                .set_icon(Some(text_icon(enabled)?))
                .map_err(|e| e.to_string())?;
            self.last_enabled = enabled;
        }
        for (name, item) in &self.levels {
            item.set_checked(name == settings.text("DetectionLevel"));
        }
        let names: Vec<String> = settings.0["Profiles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["Name"].as_str().map(str::to_owned))
            .collect();
        if !names
            .iter()
            .eq(self.profile_items.iter().map(|(name, _)| name))
        {
            for (_, item) in self.profile_items.drain(..) {
                self.profiles.remove(&item).map_err(|e| e.to_string())?;
                self.actions.retain(|(id, _)| id != item.id());
            }
            for name in names {
                let item = CheckMenuItem::new(&name, true, false, None);
                self.profiles.append(&item).map_err(|e| e.to_string())?;
                self.actions
                    .push((item.id().clone(), Action::Profile(name.clone())));
                self.profile_items.push((name, item));
            }
        }
        for (name, item) in &self.profile_items {
            item.set_checked(name == settings.text("ActiveProfile"));
        }
        Ok(())
    }
    pub fn poll(&self) -> Vec<Action> {
        self.receiver
            .try_iter()
            .filter_map(|event| match event {
                Event::Toggle => Some(Action::Toggle),
                Event::Menu(id) => self
                    .actions
                    .iter()
                    .find(|(key, _)| key == &id)
                    .map(|(_, action)| action.clone()),
            })
            .collect()
    }
}
pub fn write_action(store: &Store, action: &Action) -> Result<(), String> {
    store
        .change_settings(|settings| match action {
            Action::Toggle => {
                settings.set("Enabled", serde_json::json!(!settings.boolean("Enabled")))
            }
            Action::Profile(name) => settings.switch_profile(name),
            Action::Level(level) => settings.set("DetectionLevel", serde_json::json!(level)),
            _ => Ok(()),
        })
        .map_err(|e| e.to_string())
}
fn text_icon(enabled: bool) -> Result<Icon, String> {
    let font = [
        "C:/Windows/Fonts/YuGothB.ttc",
        "C:/Windows/Fonts/YuGothM.ttc",
        "C:/Windows/Fonts/meiryo.ttc",
        "C:/Windows/Fonts/segoeui.ttf",
    ]
    .iter()
    .find_map(|path| {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| FontArc::try_from_vec(bytes).ok())
    })
    .ok_or("tray icon font unavailable")?;
    let character = if !enabled {
        'A'
    } else if font.glyph_id('あ').0 != 0 {
        'あ'
    } else {
        'M'
    };
    let glyph = font.as_scaled(PxScale::from(15.)).scaled_glyph(character);
    let outline = font
        .outline_glyph(glyph)
        .ok_or("tray icon glyph unavailable")?;
    let bounds = outline.px_bounds();
    let mut rgba = vec![0_u8; 32 * 32 * 4];
    // Upstream CreateIcon uses a 30px accent/gray circle with white text.
    let background = if enabled {
        [0_u8, 120, 212]
    } else {
        [120_u8, 120, 120]
    };
    for y in 0..32 {
        for x in 0..32 {
            let distance = ((x as f32 + 0.5 - 16.).powi(2) + (y as f32 + 0.5 - 16.).powi(2)).sqrt();
            let alpha = ((15.5 - distance).clamp(0., 1.) * 255.) as u8;
            let i = (y * 32 + x) * 4;
            rgba[i..i + 3].copy_from_slice(&background);
            rgba[i + 3] = alpha;
        }
    }
    let left = (32. - bounds.width()) / 2.;
    let top = (32. - bounds.height()) / 2. + 1.;
    outline.draw(|x, y, coverage| {
        let x = x as usize + left.max(0.) as usize;
        let y = y as usize + top.max(0.) as usize;
        if x < 32 && y < 32 {
            let i = (y * 32 + x) * 4;
            for channel in 0..3 {
                rgba[i + channel] =
                    (background[channel] as f32 * (1. - coverage) + 255. * coverage) as u8;
            }
            rgba[i + 3] = 255;
        }
    });
    Icon::from_rgba(rgba, 32, 32).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tray_actions_write_shared_store_and_merge_with_an_open_settings_window() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let opened = store.settings().unwrap();
        let mut edited = opened.clone();
        edited
            .set("LiveConversion", serde_json::json!(false))
            .unwrap();
        write_action(&store, &Action::Toggle).unwrap();
        write_action(&store, &Action::Level("Conservative".into())).unwrap();
        let current = store.settings().unwrap();
        let merged = Settings::merge_edits(&opened, &edited, &current);
        assert!(!merged.boolean("Enabled"));
        assert!(!merged.boolean("LiveConversion"));
        assert_eq!(merged.text("DetectionLevel"), "Conservative");
    }
    #[test]
    fn both_tray_icon_glyphs_render_on_windows() {
        assert!(text_icon(true).is_ok());
        assert!(text_icon(false).is_ok());
    }
}
