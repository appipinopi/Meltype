// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
use super::*;
impl State {
    pub(super) unsafe fn paint(&mut self) {
        if self.target.is_none() {
            self.target = self.graphics.target(self.window, self.scale);
        }
        let Some(target) = self.target.clone() else {
            return;
        };
        let p = palette();
        let candidate = self.graphics.format(15.);
        let note = self.graphics.format(12.);
        target.BeginDraw();
        target.Clear(Some(&p.background));
        let mut y = 4.;
        if !self.view.typed.is_empty() {
            self.graphics.text(
                &target,
                note.as_ref(),
                &self.view.typed,
                rect(16., y, self.width - 12., y + 22.),
                p.secondary,
                false,
            );
            y += 22.;
        }
        if !self.view.suggestion.is_empty() {
            fill(
                &target,
                rect(4., y, self.width - 4., y + 32.),
                4.,
                p.suggestion,
            );
            self.graphics.text(
                &target,
                candidate.as_ref(),
                &self.view.suggestion,
                rect(16., y, self.width - 12., y + 32.),
                p.accent,
                false,
            );
            y += 32.;
            if !self.view.candidates.is_empty() {
                if let Ok(brush) = target.CreateSolidColorBrush(&p.separator, None) {
                    target.DrawLine(
                        point(8., y + 2.5),
                        point(self.width - 8., y + 2.5),
                        &brush,
                        1.,
                        None,
                    );
                }
                y += 5.;
            }
        }
        for row in 0..self.rows() {
            let index = self.first + row;
            if index as i32 == self.view.selected {
                fill(
                    &target,
                    rect(4., y, self.width - 4., y + 32.),
                    4.,
                    p.selected,
                );
                fill(&target, rect(4., y + 8., 7., y + 24.), 1.5, p.accent);
            } else if index as i32 == self.hover {
                fill(&target, rect(4., y, self.width - 4., y + 32.), 4., p.hover);
            }
            if !self.view.prediction {
                self.graphics.text(
                    &target,
                    note.as_ref(),
                    &(row + 1).to_string(),
                    rect(14., y, 40., y + 32.),
                    p.secondary,
                    false,
                );
            }
            let mut note_width = 0.;
            if let Some(text) = self.view.notes.get(index).filter(|s| !s.is_empty()) {
                note_width = self.graphics.width(note.as_ref(), text);
                self.graphics.text(
                    &target,
                    note.as_ref(),
                    text,
                    rect(self.width - 14. - note_width, y, self.width - 10., y + 32.),
                    p.secondary,
                    false,
                );
                note_width += 16.;
            }
            self.graphics.text(
                &target,
                candidate.as_ref(),
                &self.view.candidates[index],
                rect(40., y, self.width - 12. - note_width, y + 32.),
                p.text,
                false,
            );
            y += 32.;
        }
        if self.paged() {
            y = 4. + self.header() + 9. * 32.;
            let text = format!(
                "{} / {}",
                self.view.selected.max(0) + 1,
                self.view.candidates.len()
            );
            let width = self.graphics.width(note.as_ref(), &text);
            self.graphics.text(
                &target,
                note.as_ref(),
                &text,
                rect(self.width - 16. - width, y, self.width - 12., y + 24.),
                p.secondary,
                false,
            );
        }
        if target.EndDraw(None, None).is_err() {
            self.target = None;
        }
    }
    unsafe fn meaning_size(&self) -> (f32, f32) {
        let Some(format) = self.graphics.format(13.) else {
            return (0., 0.);
        };
        let _ = format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP);
        let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR);
        let Some(layout) = self
            .graphics
            .layout(&format, &self.view.meaning, 320., 2000.)
        else {
            return (24., 20.);
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        let _ = layout.GetMetrics(&mut metrics);
        (
            metrics.widthIncludingTrailingWhitespace + 24.,
            metrics.height + 20.,
        )
    }
    pub(super) unsafe fn show_meaning(&mut self) {
        let _ = KillTimer(self.window, 1);
        if self.view.meaning.is_empty() || !IsWindowVisible(self.window).as_bool() {
            return;
        }
        if self.meaning_window.is_invalid() {
            let Some(window) = self.create_window(true) else {
                return;
            };
            self.meaning_window = window;
        }
        self.place_meaning();
        let _ = ShowWindow(self.meaning_window, SW_SHOWNOACTIVATE);
        let _ = InvalidateRect(self.meaning_window, None, false);
    }
    pub(super) unsafe fn place_meaning(&mut self) {
        let (width, height) = self.meaning_size();
        let width = ((width + 2.) * self.scale).ceil() as i32;
        let height = ((height + 2.) * self.scale).ceil() as i32;
        let mut own = RECT::default();
        let _ = GetWindowRect(self.window, &mut own);
        let row = (self.view.selected - self.first as i32).max(0);
        let mut x = own.right + (4. * self.scale) as i32;
        let y = own.top + ((4. + self.header() + row as f32 * 32.) * self.scale) as i32;
        let area = work_area(&own);
        if x + width > area.right {
            x = own.left - width - (4. * self.scale) as i32;
        }
        let y = y.clamp(area.top, area.top.max(area.bottom - height));
        self.meaning_target = None;
        let _ = SetWindowPos(
            self.meaning_window,
            HWND_TOPMOST,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE,
        );
    }
    pub(super) unsafe fn paint_meaning(&mut self) {
        if self.meaning_target.is_none() {
            self.meaning_target = self.graphics.target(self.meaning_window, self.scale);
        }
        let Some(target) = self.meaning_target.clone() else {
            return;
        };
        let format = self.graphics.format(13.);
        if let Some(format) = &format {
            let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR);
        }
        let mut client = RECT::default();
        let _ = GetClientRect(self.meaning_window, &mut client);
        let p = palette();
        target.BeginDraw();
        target.Clear(Some(&p.background));
        self.graphics.text(
            &target,
            format.as_ref(),
            &self.view.meaning,
            rect(
                12.,
                10.,
                client.right as f32 / self.scale - 12.,
                client.bottom as f32 / self.scale - 10.,
            ),
            p.text,
            true,
        );
        if target.EndDraw(None, None).is_err() {
            self.meaning_target = None;
        }
    }
}
