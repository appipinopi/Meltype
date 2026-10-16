// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
use super::*;
use windows::Win32::{
    Graphics::{Dwm::*, Dxgi::Common::DXGI_FORMAT_UNKNOWN},
    System::Registry::*,
};

pub(super) struct Graphics {
    pub d2d: ID2D1Factory,
    pub write: IDWriteFactory,
}
impl Graphics {
    pub unsafe fn new() -> windows::core::Result<Self> {
        Ok(Self {
            d2d: D2D1CreateFactory(D2D1_FACTORY_TYPE_MULTI_THREADED, None)?,
            write: DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?,
        })
    }
    pub unsafe fn format(&self, size: f32) -> Option<IDWriteTextFormat> {
        let value = self
            .write
            .CreateTextFormat(
                w!("Yu Gothic UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("ja-jp"),
            )
            .ok()?;
        let _ = value.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
        let _ = value.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
        Some(value)
    }
    pub unsafe fn layout(
        &self,
        format: &IDWriteTextFormat,
        text: &str,
        width: f32,
        height: f32,
    ) -> Option<IDWriteTextLayout> {
        self.write
            .CreateTextLayout(
                &text.encode_utf16().collect::<Vec<_>>(),
                format,
                width,
                height,
            )
            .ok()
    }
    pub unsafe fn width(&self, format: Option<&IDWriteTextFormat>, text: &str) -> f32 {
        if text.is_empty() {
            return 0.;
        }
        let Some(layout) = format.and_then(|f| self.layout(f, text, 4096., 100.)) else {
            return 0.;
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        let _ = layout.GetMetrics(&mut metrics);
        metrics.widthIncludingTrailingWhitespace
    }
    pub unsafe fn text(
        &self,
        target: &ID2D1HwndRenderTarget,
        format: Option<&IDWriteTextFormat>,
        text: &str,
        rect: D2D_RECT_F,
        color: D2D1_COLOR_F,
        wrap: bool,
    ) {
        if text.is_empty() {
            return;
        }
        let Some(layout) = format
            .and_then(|f| self.layout(f, text, rect.right - rect.left, rect.bottom - rect.top))
        else {
            return;
        };
        if wrap {
            let _ = layout.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP);
        }
        if let Ok(brush) = target.CreateSolidColorBrush(&color, None) {
            target.DrawTextLayout(
                point(rect.left, rect.top),
                &layout,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT | D2D1_DRAW_TEXT_OPTIONS_CLIP,
            );
        }
    }
    pub unsafe fn target(&self, window: HWND, scale: f32) -> Option<ID2D1HwndRenderTarget> {
        let mut client = RECT::default();
        GetClientRect(window, &mut client).ok()?;
        let target = self
            .d2d
            .CreateHwndRenderTarget(
                &D2D1_RENDER_TARGET_PROPERTIES {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_UNKNOWN,
                        alphaMode: D2D1_ALPHA_MODE_UNKNOWN,
                    },
                    dpiX: scale * 96.,
                    dpiY: scale * 96.,
                    ..Default::default()
                },
                &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd: window,
                    pixelSize: D2D_SIZE_U {
                        width: client.right.max(1) as u32,
                        height: client.bottom.max(1) as u32,
                    },
                    ..Default::default()
                },
            )
            .ok()?;
        target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        Some(target)
    }
}
pub(super) fn rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}
pub(super) fn point(x: f32, y: f32) -> D2D_POINT_2F {
    D2D_POINT_2F { x, y }
}
pub(super) unsafe fn fill(
    target: &ID2D1HwndRenderTarget,
    rect: D2D_RECT_F,
    radius: f32,
    color: D2D1_COLOR_F,
) {
    if let Ok(brush) = target.CreateSolidColorBrush(&color, None) {
        target.FillRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect,
                radiusX: radius,
                radiusY: radius,
            },
            &brush,
        );
    }
}
pub(super) struct Palette {
    pub background: D2D1_COLOR_F,
    pub text: D2D1_COLOR_F,
    pub secondary: D2D1_COLOR_F,
    pub selected: D2D1_COLOR_F,
    pub hover: D2D1_COLOR_F,
    pub accent: D2D1_COLOR_F,
    pub separator: D2D1_COLOR_F,
    pub suggestion: D2D1_COLOR_F,
    pub border: COLORREF,
}
fn color(r: u32, g: u32, b: u32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.,
        g: g as f32 / 255.,
        b: b as f32 / 255.,
        a,
    }
}
unsafe fn setting(path: windows::core::PCWSTR, name: windows::core::PCWSTR, fallback: u32) -> u32 {
    let mut value = fallback;
    let mut size = 4;
    let _ = RegGetValueW(
        HKEY_CURRENT_USER,
        path,
        name,
        RRF_RT_REG_DWORD,
        None,
        Some(ptr::addr_of_mut!(value).cast()),
        Some(&mut size),
    );
    value
}
pub(super) unsafe fn palette() -> Palette {
    let accent = setting(
        w!("Software\\Microsoft\\Windows\\DWM"),
        w!("AccentColor"),
        0x00d47800,
    );
    let (r, g, b) = (accent & 255, (accent >> 8) & 255, (accent >> 16) & 255);
    if setting(
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
        w!("AppsUseLightTheme"),
        1,
    ) == 0
    {
        let light = |v| v + (255 - v) * 2 / 5;
        Palette {
            background: color(44, 44, 44, 1.),
            text: color(255, 255, 255, 1.),
            secondary: color(255, 255, 255, 0.62),
            selected: color(255, 255, 255, 0.08),
            hover: color(255, 255, 255, 0.05),
            accent: color(light(r), light(g), light(b), 1.),
            separator: color(255, 255, 255, 0.08),
            suggestion: color(light(r), light(g), light(b), 0.12),
            border: COLORREF(0x464646),
        }
    } else {
        Palette {
            background: color(249, 249, 249, 1.),
            text: color(26, 26, 26, 1.),
            secondary: color(0, 0, 0, 0.6),
            selected: color(0, 0, 0, 0.06),
            hover: color(0, 0, 0, 0.035),
            accent: color(r, g, b, 1.),
            separator: color(0, 0, 0, 0.08),
            suggestion: color(r, g, b, 0.08),
            border: COLORREF(0xdedede),
        }
    }
}
pub(super) unsafe fn prepare(window: HWND) {
    let corner = 2u32;
    let _ = DwmSetWindowAttribute(
        window,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        ptr::addr_of!(corner).cast(),
        4,
    );
    let border = palette().border;
    let _ = DwmSetWindowAttribute(window, DWMWA_BORDER_COLOR, ptr::addr_of!(border).cast(), 4);
}
pub(super) unsafe fn work_area(anchor: &RECT) -> RECT {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = GetMonitorInfoW(MonitorFromRect(anchor, MONITOR_DEFAULTTONEAREST), &mut info);
    info.rcWork
}
pub(super) unsafe fn place(x: &mut i32, y: &mut i32, width: i32, height: i32, anchor: &RECT) {
    let area = work_area(anchor);
    if *y + height > area.bottom {
        *y = anchor.top - height - 4;
    }
    *y = (*y).max(area.top);
    if *x + width > area.right {
        *x = area.right - width;
    }
    *x = (*x).max(area.left);
}
