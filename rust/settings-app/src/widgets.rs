// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 amanetoki7, appipinopi
use eframe::egui;
pub fn wheel_step(ui: &egui::Ui, response: &egui::Response, enabled: bool) -> i64 {
    if !enabled || !response.hovered() {
        return 0;
    }
    ui.input_mut(|i| {
        let delta = i.smooth_scroll_delta.y;
        if delta.abs() < 1. {
            return 0;
        }
        i.smooth_scroll_delta.y = 0.;
        i.raw_scroll_delta.y = 0.;
        if delta > 0. {
            1
        } else {
            -1
        }
    })
}
/// Same 40×20 track, ON/OFF text and keyboard role as upstream ToggleSwitch.
pub fn toggle(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(egui::vec2(88., 28.), egui::Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
    });
    let track = egui::Rect::from_min_size(rect.min + egui::vec2(4., 4.), egui::vec2(40., 20.));
    let accent = egui::Color32::from_rgb(0, 120, 212);
    let gray = ui.visuals().weak_text_color();
    ui.painter().rect_filled(
        track,
        10.,
        if *value {
            accent
        } else {
            ui.visuals().panel_fill
        },
    );
    if !*value {
        ui.painter().rect_stroke(
            track,
            10.,
            egui::Stroke::new(1_f32, gray),
            egui::StrokeKind::Inside,
        );
    }
    let radius = if response.hovered() { 7. } else { 6. };
    let center = egui::pos2(
        if *value {
            track.right() - 10.
        } else {
            track.left() + 10.
        },
        track.center().y,
    );
    ui.painter().circle_filled(
        center,
        radius,
        if *value { egui::Color32::WHITE } else { gray },
    );
    ui.painter().text(
        egui::pos2(track.right() + 12., track.center().y),
        egui::Align2::LEFT_CENTER,
        if *value { "ON" } else { "OFF" },
        egui::FontId::proportional(13.),
        ui.visuals().text_color(),
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            2.,
            egui::Stroke::new(1_f32, accent),
            egui::StrokeKind::Inside,
        );
    }
    response
}
