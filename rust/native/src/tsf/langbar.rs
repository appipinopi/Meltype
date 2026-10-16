// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! Rust implementation of the TSF input-mode button, its source interface,
//! update subscription and taskbar-theme-aware icon.
use std::{
    ffi::c_void,
    mem::MaybeUninit,
    ptr,
    sync::{
        atomic::{fence, AtomicU32, Ordering},
        Mutex,
    },
    thread::{self, ThreadId},
};
use windows::{
    core::{w, IUnknown, IUnknown_Vtbl, Interface, BSTR, GUID, HRESULT},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{
            Ole::{CONNECT_E_ADVISELIMIT, CONNECT_E_CANNOTCONNECT, CONNECT_E_NOCONNECTION},
            Registry::*,
        },
        UI::{TextServices::*, WindowsAndMessaging::*},
    },
};

type ReadMode = unsafe extern "C" fn(*mut c_void) -> i32;
type Toggle = unsafe extern "C" fn(*mut c_void);
#[derive(Clone, Copy)]
struct Client {
    context: *mut c_void,
    read: ReadMode,
    toggle: Toggle,
}
struct State {
    client: Option<Client>,
    sink: Option<ITfLangBarItemSink>,
}
#[repr(C)]
struct Bar {
    button: *const ITfLangBarItemButton_Vtbl,
    source: *const ITfSource_Vtbl,
    refs: AtomicU32,
    owner: ThreadId,
    state: Mutex<State>,
}
const COOKIE: u32 = 0x4d454c54;
unsafe fn bar<'a>(this: *mut c_void) -> &'a Bar {
    &*this.cast::<Bar>()
}
unsafe fn from_source(this: *mut c_void) -> *mut c_void {
    this.cast::<usize>().sub(1).cast()
}
unsafe extern "system" fn query(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    out.write(ptr::null_mut());
    if iid.is_null() {
        return E_INVALIDARG;
    }
    if [
        IUnknown::IID,
        ITfLangBarItem::IID,
        ITfLangBarItemButton::IID,
    ]
    .contains(&*iid)
    {
        out.write(this);
    } else if *iid == ITfSource::IID {
        out.write(ptr::addr_of!(bar(this).source).cast_mut().cast());
    } else {
        return E_NOINTERFACE;
    }
    add_ref(this);
    S_OK
}
unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    bar(this).refs.fetch_add(1, Ordering::Relaxed) + 1
}
unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let count = bar(this).refs.fetch_sub(1, Ordering::Release) - 1;
    if count == 0 {
        fence(Ordering::Acquire);
        drop(Box::from_raw(this.cast::<Bar>()));
    }
    count
}
unsafe extern "system" fn source_query(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    query(from_source(this), iid, out)
}
unsafe extern "system" fn source_add_ref(this: *mut c_void) -> u32 {
    add_ref(from_source(this))
}
unsafe extern "system" fn source_release(this: *mut c_void) -> u32 {
    release(from_source(this))
}
fn client(bar: &Bar) -> Option<Client> {
    if bar.owner != thread::current().id() {
        return None;
    }
    bar.state.lock().unwrap_or_else(|e| e.into_inner()).client
}
unsafe fn is_open(this: *mut c_void) -> bool {
    client(bar(this)).is_none_or(|c| (c.read)(c.context) != 0)
}
unsafe fn string(out: *mut MaybeUninit<BSTR>, text: &str) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    out.write(MaybeUninit::new(BSTR::new()));
    match BSTR::from_wide(&text.encode_utf16().collect::<Vec<_>>()) {
        Ok(value) => {
            out.write(MaybeUninit::new(value));
            S_OK
        }
        Err(e) => e.code(),
    }
}
unsafe extern "system" fn info(_this: *mut c_void, out: *mut TF_LANGBARITEMINFO) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    let mut value = TF_LANGBARITEMINFO {
        clsidService: GUID::from_u128(0x417d801b_a9bd_4c26_bd16_356a825a6998),
        guidItem: GUID_LBI_INPUTMODE,
        dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY,
        ..Default::default()
    };
    for (to, from) in value
        .szDescription
        .iter_mut()
        .zip("Meltype の入力モード".encode_utf16())
    {
        *to = from;
    }
    out.write(value);
    S_OK
}
unsafe extern "system" fn status(_this: *mut c_void, out: *mut u32) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    out.write(0);
    S_OK
}
unsafe extern "system" fn show(_this: *mut c_void, _show: BOOL) -> HRESULT {
    E_NOTIMPL
}
unsafe extern "system" fn tooltip(this: *mut c_void, out: *mut MaybeUninit<BSTR>) -> HRESULT {
    string(
        out,
        if is_open(this) {
            "Meltype: 日本語 (クリックで英数)"
        } else {
            "Meltype: 英数 (クリックで日本語)"
        },
    )
}
unsafe extern "system" fn text(this: *mut c_void, out: *mut MaybeUninit<BSTR>) -> HRESULT {
    string(out, if is_open(this) { "あ" } else { "A" })
}
unsafe extern "system" fn click(
    this: *mut c_void,
    _click: TfLBIClick,
    _point: POINT,
    _area: *const RECT,
) -> HRESULT {
    // Do not hold the lock across a callback: changing the mode can call Update
    // or Deactivate/Detach synchronously from the same TSF thread.
    if let Some(c) = client(bar(this)) {
        (c.toggle)(c.context);
    }
    S_OK
}
unsafe extern "system" fn init_menu(_this: *mut c_void, _menu: *mut c_void) -> HRESULT {
    S_OK
}
unsafe extern "system" fn select_menu(_this: *mut c_void, _id: u32) -> HRESULT {
    S_OK
}
unsafe extern "system" fn icon(this: *mut c_void, out: *mut HICON) -> HRESULT {
    if out.is_null() {
        return E_INVALIDARG;
    }
    out.write(HICON::default());
    match create_icon(is_open(this)) {
        Ok(value) => {
            out.write(value);
            S_OK
        }
        Err(e) => e.code(),
    }
}
unsafe extern "system" fn advise(
    this: *mut c_void,
    iid: *const GUID,
    sink: *mut c_void,
    cookie: *mut u32,
) -> HRESULT {
    if cookie.is_null() {
        return E_INVALIDARG;
    }
    cookie.write(u32::MAX);
    if iid.is_null() || sink.is_null() {
        return E_INVALIDARG;
    }
    if *iid != ITfLangBarItemSink::IID {
        return CONNECT_E_CANNOTCONNECT;
    }
    let b = bar(from_source(this));
    if b.owner != thread::current().id() {
        return RPC_E_WRONG_THREAD;
    }
    if b.state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .sink
        .is_some()
    {
        return CONNECT_E_ADVISELIMIT;
    }
    let mut value = ptr::null_mut();
    let unknown = &*sink.cast::<*const IUnknown_Vtbl>();
    let result = ((**unknown).QueryInterface)(sink, &ITfLangBarItemSink::IID, &mut value);
    if result.is_err() {
        return result;
    }
    if value.is_null() {
        return E_NOINTERFACE;
    }
    let value = ITfLangBarItemSink::from_raw(value);
    let mut state = b.state.lock().unwrap_or_else(|e| e.into_inner());
    if state.sink.is_some() {
        drop(state);
        drop(value);
        return CONNECT_E_ADVISELIMIT;
    }
    state.sink = Some(value);
    cookie.write(COOKIE);
    S_OK
}
unsafe extern "system" fn unadvise(this: *mut c_void, cookie: u32) -> HRESULT {
    let b = bar(from_source(this));
    if b.owner != thread::current().id() {
        return RPC_E_WRONG_THREAD;
    }
    let mut state = b.state.lock().unwrap_or_else(|e| e.into_inner());
    if cookie != COOKIE || state.sink.is_none() {
        return CONNECT_E_NOCONNECTION;
    }
    let sink = state.sink.take();
    drop(state);
    drop(sink);
    S_OK
}
static BUTTON: ITfLangBarItemButton_Vtbl = ITfLangBarItemButton_Vtbl {
    base__: ITfLangBarItem_Vtbl {
        base__: IUnknown_Vtbl {
            QueryInterface: query,
            AddRef: add_ref,
            Release: release,
        },
        GetInfo: info,
        GetStatus: status,
        Show: show,
        GetTooltipString: tooltip,
    },
    OnClick: click,
    InitMenu: init_menu,
    OnMenuSelect: select_menu,
    GetIcon: icon,
    GetText: text,
};
static SOURCE: ITfSource_Vtbl = ITfSource_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: source_query,
        AddRef: source_add_ref,
        Release: source_release,
    },
    AdviseSink: advise,
    UnadviseSink: unadvise,
};

/// Returns an owned ITfLangBarItemButton reference. Both callbacks run only on
/// the creating thread. Detach before destroying the callback context/code.
#[no_mangle]
pub extern "C" fn meltype_tsf_create_langbar(
    context: *mut c_void,
    read: Option<ReadMode>,
    toggle: Option<Toggle>,
) -> *mut c_void {
    let client = read.zip(toggle).map(|(read, toggle)| Client {
        context,
        read,
        toggle,
    });
    Box::into_raw(Box::new(Bar {
        button: &BUTTON,
        source: &SOURCE,
        refs: AtomicU32::new(1),
        owner: thread::current().id(),
        state: Mutex::new(State { client, sink: None }),
    }))
    .cast()
}
/// # Safety
/// `this` must be a live pointer returned by meltype_tsf_create_langbar.
/// Detach must run on the creating TSF thread before freeing its context/code.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_langbar_detach(this: *mut c_void) {
    if !this.is_null() {
        if bar(this).owner != thread::current().id() {
            return;
        }
        bar(this)
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .client = None;
    }
}
/// # Safety
/// `this` must be a live pointer returned by meltype_tsf_create_langbar.
#[no_mangle]
pub unsafe extern "C" fn meltype_tsf_langbar_update(this: *mut c_void) {
    if this.is_null() {
        return;
    }
    let b = bar(this);
    if b.owner != thread::current().id() {
        return;
    }
    let sink = b
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .sink
        .clone();
    if let Some(sink) = sink {
        let _ = sink.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT | TF_LBI_TOOLTIP);
    }
}

struct Canvas {
    dc: HDC,
    color: HBITMAP,
    font: HFONT,
    old_bitmap: HGDIOBJ,
    old_font: HGDIOBJ,
}
impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            if !self.old_font.is_invalid() {
                SelectObject(self.dc, self.old_font);
            }
            if !self.old_bitmap.is_invalid() {
                SelectObject(self.dc, self.old_bitmap);
            }
            if !self.font.is_invalid() {
                let _ = DeleteObject(self.font);
            }
            if !self.color.is_invalid() {
                let _ = DeleteObject(self.color);
            }
            let _ = DeleteDC(self.dc);
        }
    }
}
unsafe fn dark() -> bool {
    let mut value = 0u32;
    let mut size = 4;
    let result = RegGetValueW(
        HKEY_CURRENT_USER,
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
        w!("SystemUsesLightTheme"),
        RRF_RT_REG_DWORD,
        None,
        Some(ptr::addr_of_mut!(value).cast()),
        Some(&mut size),
    );
    result != ERROR_SUCCESS || value == 0
}
unsafe fn create_icon(open: bool) -> windows::core::Result<HICON> {
    let size = GetSystemMetrics(SM_CXSMICON);
    let size = if size <= 0 { 16 } else { size.min(512) };
    let screen = GetDC(None);
    let dc = CreateCompatibleDC(screen);
    ReleaseDC(None, screen);
    if dc.is_invalid() {
        return Err(E_FAIL.into());
    }
    let mut canvas = Canvas {
        dc,
        color: HBITMAP::default(),
        font: HFONT::default(),
        old_bitmap: HGDIOBJ::default(),
        old_font: HGDIOBJ::default(),
    };
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = ptr::null_mut();
    canvas.color = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
    if bits.is_null() {
        return Err(E_FAIL.into());
    }
    canvas.old_bitmap = SelectObject(dc, canvas.color);
    let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), (size * size * 4) as usize);
    pixels.fill(0);
    canvas.font = CreateFontW(
        -(size * 15 + 8) / 16,
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        ANTIALIASED_QUALITY.0 as u32,
        DEFAULT_PITCH.0 as u32,
        w!("Yu Gothic UI"),
    );
    if canvas.font.is_invalid() {
        return Err(E_FAIL.into());
    }
    canvas.old_font = SelectObject(dc, canvas.font);
    SetBkMode(dc, TRANSPARENT);
    SetTextColor(dc, COLORREF(0xffffff));
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: size,
        bottom: size,
    };
    let mut text: Vec<u16> = if open { "あ" } else { "A" }.encode_utf16().collect();
    DrawTextW(
        dc,
        &mut text,
        &mut rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );
    let _ = GdiFlush();
    let dark = dark();
    for pixel in pixels.as_chunks_mut::<4>().0 {
        let alpha = pixel[1];
        let value = if dark { alpha } else { 0 };
        pixel.copy_from_slice(&[value, value, value, alpha]);
    }
    let mask = CreateBitmap(size, size, 1, 1, None);
    if mask.is_invalid() {
        return Err(E_FAIL.into());
    }
    let result = CreateIconIndirect(&ICONINFO {
        fIcon: BOOL(1),
        hbmColor: canvas.color,
        hbmMask: mask,
        ..Default::default()
    });
    let _ = DeleteObject(mask);
    result
}
