// SPDX-License-Identifier: GPL-3.0-or-later
//! Registration-free buffered input. The low-level callback only classifies and
//! queues; conversion/UI Automation never blocks the system hook callback.
use meltype_core::{settings::Settings, storage::Store, Session};
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        Arc, OnceLock,
    },
    time::Duration,
};
use windows::Win32::{System::Com::*, UI::Accessibility::*};
use windows_sys::{
    core::w,
    Win32::{
        Foundation::*,
        Graphics::Gdi::ClientToScreen,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
};
const INJECTED_TAG: usize = 0x4d52504b;
#[derive(Clone, Copy)]
struct Key {
    vk: u32,
    ch: i32,
    mods: i32,
    target: usize,
    generation: u64,
}
enum Event {
    Key(Key),
    Reset,
    Settings(Box<Settings>),
}
struct Shared {
    active: AtomicBool,
    permitted: AtomicBool,
    composing: AtomicBool,
    ready: AtomicBool,
    exit: AtomicBool,
    generation: AtomicU64,
    queue: SyncSender<Event>,
    #[cfg(test)]
    test_pid: u32,
}
static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
thread_local! { static SUPPRESSED: RefCell<[bool;256]> = const { RefCell::new([false;256]) }; }
fn modifiers() -> i32 {
    unsafe {
        i32::from(GetAsyncKeyState(VK_SHIFT as i32) < 0)
            | (i32::from(GetAsyncKeyState(VK_CONTROL as i32) < 0) << 1)
            | (i32::from(GetAsyncKeyState(VK_MENU as i32) < 0) << 2)
            | (i32::from(
                GetAsyncKeyState(VK_LWIN as i32) < 0 || GetAsyncKeyState(VK_RWIN as i32) < 0,
            ) << 3)
    }
}
fn captures(vk: u32, ch: i32, composing: bool) -> bool {
    let _ = composing;
    (0x20..=0x7e).contains(&ch) || matches!(vk, 8|9|13|27|0x21..=0x28|0x2e|0x75..=0x79)
}
unsafe extern "system" fn hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    let consumed = std::panic::catch_unwind(|| {
        let Some(shared) = SHARED.get() else {
            return false;
        };
        if code < 0 || data == 0 {
            return false;
        }
        let event = &*(data as *const KBDLLHOOKSTRUCT);
        #[cfg(test)]
        let fixture = {
            let mut pid = 0;
            GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
            shared.test_pid != 0 && shared.test_pid == pid && event.dwExtraInfo == 0x4d525054
        };
        #[cfg(not(test))]
        let fixture = false;
        if (event.flags & LLKHF_INJECTED != 0 && !fixture)
            || event.dwExtraInfo == INJECTED_TAG
            || event.vkCode >= 256
        {
            return false;
        }
        let vk = event.vkCode;
        if matches!(message as u32, WM_KEYUP | WM_SYSKEYUP) {
            return SUPPRESSED
                .with(|keys| std::mem::replace(&mut keys.borrow_mut()[vk as usize], false));
        }
        if !matches!(message as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
            return false;
        }
        let mods = modifiers();
        if vk == 0x4a && mods & 14 == 6 && shared.permitted.load(Ordering::Acquire) {
            // Ignore auto-repeat; physical key-up re-arms the toggle.
            let repeated = SUPPRESSED
                .with(|keys| std::mem::replace(&mut keys.borrow_mut()[vk as usize], true));
            if !repeated {
                shared.active.fetch_xor(true, Ordering::SeqCst);
                shared.generation.fetch_add(1, Ordering::SeqCst);
                let _ = shared.queue.try_send(Event::Reset);
            }
            return true;
        }
        if !shared.active.load(Ordering::Acquire)
            || !shared.permitted.load(Ordering::Acquire)
            || !shared.ready.load(Ordering::Acquire)
        {
            return false;
        }
        if matches!(vk,0x10..=0x12|0x5b..=0x5c|0xa0..=0xa5) {
            return false;
        }
        if mods & 14 != 0 {
            shared.generation.fetch_add(1, Ordering::SeqCst);
            let _ = shared.queue.try_send(Event::Reset);
            return false;
        }
        let target = GetForegroundWindow();
        if target.is_null() {
            return false;
        }
        let thread = GetWindowThreadProcessId(target, std::ptr::null_mut());
        let mut state = [0u8; 256];
        GetKeyboardState(state.as_mut_ptr());
        state[VK_SHIFT as usize] = if mods & 1 != 0 { 0x80 } else { 0 };
        state[vk as usize] = 0x80;
        let mut text = [0u16; 8];
        let count = ToUnicodeEx(
            vk,
            event.scanCode,
            state.as_ptr(),
            text.as_mut_ptr(),
            text.len() as i32,
            4,
            GetKeyboardLayout(thread),
        );
        let ch = if count == 1 { text[0] as i32 } else { 0 };
        if !captures(vk, ch, shared.composing.load(Ordering::Acquire)) {
            return false;
        }
        let key = Key {
            vk,
            ch,
            mods,
            target: target as usize,
            generation: shared.generation.load(Ordering::Acquire),
        };
        if shared.queue.try_send(Event::Key(key)).is_err() {
            shared.active.store(false, Ordering::Release);
            shared.generation.fetch_add(1, Ordering::SeqCst);
            return false;
        }
        if ch >= 0x20 {
            shared.composing.store(true, Ordering::Release);
        }
        SUPPRESSED.with(|keys| keys.borrow_mut()[vk as usize] = true);
        true
    })
    .unwrap_or(false);
    if consumed {
        1
    } else {
        CallNextHookEx(std::ptr::null_mut(), code, message, data)
    }
}
fn virtual_key(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: INJECTED_TAG,
            },
        },
    }
}
fn unicode(text: &str) -> Vec<INPUT> {
    text.encode_utf16()
        .flat_map(|unit| {
            [false, true].map(|up| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0,
                        wScan: unit,
                        dwFlags: KEYEVENTF_UNICODE | if up { KEYEVENTF_KEYUP } else { 0 },
                        time: 0,
                        dwExtraInfo: INJECTED_TAG,
                    },
                },
            })
        })
        .collect()
}
unsafe fn send(inputs: &[INPUT], target: usize) -> bool {
    !inputs.is_empty()
        && GetForegroundWindow() as usize == target
        && SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        ) == inputs.len() as u32
}
unsafe fn writable(automation: &IUIAutomation) -> Option<IUIAutomationElement> {
    let element = automation.GetFocusedElement().ok()?;
    let value_readonly = element
        .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
        .ok()
        .and_then(|value| value.CurrentIsReadOnly().ok())
        .map(|v| v.as_bool());
    let text_readonly = || {
        element
            .GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            .ok()
            .and_then(|pattern| pattern.DocumentRange().ok())
            .and_then(|range| range.GetAttributeValue(UIA_IsReadOnlyAttributeId).ok())
            .and_then(read_only_bool)
    };
    let readonly = value_readonly.or_else(text_readonly);
    if !element.CurrentIsPassword().is_ok_and(|v| !v.as_bool())
        || !element.CurrentIsEnabled().is_ok_and(|v| v.as_bool())
        || readonly == Some(true)
    {
        return None;
    }
    let kind = element.CurrentControlType().ok()?;
    let focusable = element
        .CurrentIsKeyboardFocusable()
        .is_ok_and(|v| v.as_bool());
    let editable = readonly == Some(false)
        && (kind == UIA_EditControlTypeId || kind == UIA_DocumentControlTypeId || focusable)
        || focusable && ia2_editable(&element);
    editable.then_some(element)
}

// Chromium/Electron can expose an empty contenteditable as a Group with no
// Value/TextPattern. Match the original host's IAccessible2 EDITABLE fallback.
// Never read the accessible value or document text here.
unsafe fn ia2_editable(element: &IUIAutomationElement) -> bool {
    use windows::core::{IUnknown, Interface, GUID, HRESULT};
    #[repr(C)]
    struct Ia2Vtable {
        unknown: windows::core::IUnknown_Vtbl,
        // IDispatch + IAccessible + IAccessible2 methods before get_states.
        before_states: [usize; 32],
        states: unsafe extern "system" fn(*mut std::ffi::c_void, *mut i32) -> HRESULT,
    }
    let query = || -> windows::core::Result<bool> {
        let legacy = element.GetCurrentPatternAs::<IUIAutomationLegacyIAccessiblePattern>(
            UIA_LegacyIAccessiblePatternId,
        )?;
        // MSAA READONLY / PROTECTED must override the IA2 fallback.
        if legacy.CurrentState()? & (0x40 | 0x20000000) != 0 {
            return Ok(false);
        }
        let accessible = legacy.GetIAccessible()?;
        let provider: IServiceProvider = accessible.cast()?;
        let service = GUID::from_u128(0x618736e0_3c3d_11cf_810c_00aa00389b71);
        let iid = GUID::from_u128(0xe89f726e_c4f4_4c19_bb19_b647d7fa8478);
        let mut raw = std::ptr::null_mut();
        (provider.vtable().QueryService)(provider.as_raw(), &service, &iid, &mut raw).ok()?;
        if raw.is_null() {
            return Ok(false);
        }
        // Own the queried interface so every return path releases it.
        let ia2 = IUnknown::from_raw(raw);
        let vtable = &**(ia2.as_raw() as *const *const Ia2Vtable);
        let mut states = 0;
        (vtable.states)(ia2.as_raw(), &mut states).ok()?;
        Ok(states & 0x8 != 0)
    };
    query().unwrap_or(false)
}

fn read_only_bool(value: windows::core::VARIANT) -> Option<bool> {
    // UIA's NotSupported/MixedAttribute sentinels must never coerce to false.
    if unsafe { value.as_raw().Anonymous.Anonymous.vt }
        != windows::Win32::System::Variant::VT_BOOL.0
    {
        return None;
    }
    bool::try_from(&value).ok()
}
fn invalidate() {
    if let Some(state) = SHARED.get() {
        state.ready.store(false, Ordering::Release);
        state.generation.fetch_add(1, Ordering::SeqCst);
        let _ = state.queue.try_send(Event::Reset);
    }
}
unsafe extern "system" fn focus_changed(
    _: windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK,
    _: u32,
    _: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    invalidate();
}
unsafe extern "system" fn mouse_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0
        && matches!(
            message as u32,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_MOUSEWHEEL
        )
    {
        invalidate();
    }
    CallNextHookEx(std::ptr::null_mut(), code, message, data)
}
pub struct Keyboard {
    shared: Arc<Shared>,
    worker: Option<std::thread::JoinHandle<()>>,
    last_settings: Option<Settings>,
}
impl Keyboard {
    pub fn enabled(&self) -> bool {
        self.shared.active.load(Ordering::Acquire)
    }
    pub fn activate(&self) {
        if self.shared.permitted.load(Ordering::Acquire) {
            self.shared.active.store(true, Ordering::Release);
            self.shared.generation.fetch_add(1, Ordering::SeqCst);
            let _ = self.shared.queue.try_send(Event::Reset);
        }
    }
    pub fn configure(&mut self, settings: &Settings) {
        self.shared.permitted.store(
            settings.boolean("Enabled") && settings.text("Mode") == "Keyboard",
            Ordering::Release,
        );
        if self
            .last_settings
            .as_ref()
            .is_some_and(|s| s.0 == settings.0)
        {
            return;
        }
        self.shared.generation.fetch_add(1, Ordering::SeqCst);
        if self
            .shared
            .queue
            .try_send(Event::Settings(Box::new(settings.clone())))
            .is_ok()
        {
            self.last_settings = Some(settings.clone());
        }
    }
    pub fn start(store: Store) -> Result<Self, String> {
        let (queue, receiver) = mpsc::sync_channel::<Event>(128);
        let shared = Arc::new(Shared {
            active: AtomicBool::new(false),
            permitted: AtomicBool::new(
                store
                    .settings()
                    .map(|s| s.boolean("Enabled") && s.text("Mode") == "Keyboard")
                    .unwrap_or(false),
            ),
            composing: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            exit: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            queue,
            #[cfg(test)]
            test_pid: std::env::var("MELTYPE_PORTABLE_TEST_PID")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        });
        SHARED
            .set(shared.clone())
            .map_err(|_| "ポータブル入力は既に起動しています")?;
        let state = shared.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || unsafe {
            let init = (|| -> Result<(Session, IUIAutomation), String> {
                CoInitializeEx(None, COINIT_MULTITHREADED)
                    .ok()
                    .map_err(|e| e.to_string())?;
                let automation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| e.to_string())?;
                let mut session = Session::open(store).map_err(|e| e.to_string())?;
                session.can_delete = false;
                session.no_learning = true;
                session
                    .settings
                    .set("LiveConversion", serde_json::json!(false))?;
                // A deliberately bounded key path; real OS conversion belongs to TSF.
                session.display(false);
                Ok((session, automation))
            })();
            let (mut session, automation) = match init {
                Ok(value) => value,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            let hook_state = state.clone();
            let (hook_tx, hook_rx) = mpsc::sync_channel(1);
            let hook_thread = std::thread::spawn(move || {
                let handle = SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(hook),
                    GetModuleHandleW(std::ptr::null()),
                    0,
                );
                if handle.is_null() {
                    let _ = hook_tx.send(false);
                    return;
                }
                let mouse = SetWindowsHookExW(
                    WH_MOUSE_LL,
                    Some(mouse_hook),
                    GetModuleHandleW(std::ptr::null()),
                    0,
                );
                let focus = windows_sys::Win32::UI::Accessibility::SetWinEventHook(
                    EVENT_OBJECT_FOCUS,
                    EVENT_OBJECT_FOCUS,
                    std::ptr::null_mut(),
                    Some(focus_changed),
                    0,
                    0,
                    0,
                );
                let _ = hook_tx.send(!mouse.is_null() && !focus.is_null());
                if mouse.is_null() || focus.is_null() {
                    hook_state.exit.store(true, Ordering::Release);
                }
                let mut message = MSG::default();
                while !hook_state.exit.load(Ordering::Acquire) {
                    while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                UnhookWindowsHookEx(handle);
                if !mouse.is_null() {
                    UnhookWindowsHookEx(mouse);
                }
                if !focus.is_null() {
                    windows_sys::Win32::UI::Accessibility::UnhookWinEvent(focus);
                }
            });
            if hook_rx.recv_timeout(Duration::from_secs(5)) != Ok(true) {
                let _ = ready_tx.send(Err("キーボードフックを登録できません".into()));
                state.exit.store(true, Ordering::Release);
                let _ = hook_thread.join();
                return;
            }
            let overlay = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("Meltype portable"),
                WS_POPUP | WS_BORDER | 0x80,
                20,
                40,
                700,
                100,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );
            if overlay.is_null() {
                state.exit.store(true, Ordering::Release);
                let _ = ready_tx.send(Err("候補表示を作成できません".into()));
                let _ = hook_thread.join();
                CoUninitialize();
                return;
            }
            let _ = ready_tx.send(Ok(()));
            let mut target = 0usize;
            let mut focused: Option<IUIAutomationElement> = None;
            let mut message = MSG::default();
            while !state.exit.load(Ordering::Acquire) {
                while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                if !state.active.load(Ordering::Acquire)
                    || !state.permitted.load(Ordering::Acquire)
                    || GetForegroundWindow() as usize != target
                {
                    session.reset();
                    state.composing.store(false, Ordering::Release);
                    ShowWindow(overlay, SW_HIDE);
                }
                let current = if state.active.load(Ordering::Acquire)
                    && state.permitted.load(Ordering::Acquire)
                {
                    writable(&automation)
                } else {
                    None
                };
                let same = focused
                    .as_ref()
                    .zip(current.as_ref())
                    .is_some_and(|(a, b)| {
                        automation.CompareElements(a, b).is_ok_and(|v| v.as_bool())
                    });
                if !same {
                    session.reset();
                    ShowWindow(overlay, SW_HIDE);
                }
                state.ready.store(current.is_some(), Ordering::Release);
                focused = current;
                let Ok(event) = receiver.recv_timeout(Duration::from_millis(20)) else {
                    continue;
                };
                let key = match event {
                    Event::Settings(settings) => {
                        session.reset();
                        session.settings = *settings;
                        let _ = session
                            .settings
                            .set("LiveConversion", serde_json::json!(false));
                        ShowWindow(overlay, SW_HIDE);
                        continue;
                    }
                    Event::Reset => {
                        state.composing.store(false, Ordering::Release);
                        session.reset();
                        ShowWindow(overlay, SW_HIDE);
                        continue;
                    }
                    Event::Key(key) => key,
                };
                if key.generation != state.generation.load(Ordering::Acquire)
                    || !state.active.load(Ordering::Acquire)
                    || !state.permitted.load(Ordering::Acquire)
                    || GetForegroundWindow() as usize != key.target
                {
                    continue;
                }
                if target != key.target {
                    session.reset();
                    target = key.target;
                }
                if !focused
                    .as_ref()
                    .zip(writable(&automation).as_ref())
                    .is_some_and(|(a, b)| {
                        automation.CompareElements(a, b).is_ok_and(|v| v.as_bool())
                    })
                {
                    session.reset();
                    state.active.store(false, Ordering::Release);
                    let _ = send(
                        &[
                            virtual_key(key.vk as u16, false),
                            virtual_key(key.vk as u16, true),
                        ],
                        target,
                    );
                    continue;
                }
                let reply = session.key(key.vk as i32, key.ch, key.mods);
                if reply.commits.iter().any(|edit| {
                    edit.delete_before != 0
                        || edit.caret_back as usize > edit.text.encode_utf16().count()
                }) {
                    session.reset();
                    state.active.store(false, Ordering::Release);
                    continue;
                }
                if key.generation != state.generation.load(Ordering::Acquire)
                    || !focused
                        .as_ref()
                        .zip(writable(&automation).as_ref())
                        .is_some_and(|(a, b)| {
                            automation.CompareElements(a, b).is_ok_and(|v| v.as_bool())
                        })
                {
                    session.reset();
                    continue;
                }
                let inputs: Vec<INPUT> = reply
                    .commits
                    .iter()
                    .flat_map(|edit| {
                        let mut inputs = unicode(&edit.text);
                        for _ in 0..edit.caret_back {
                            inputs
                                .extend([virtual_key(VK_LEFT, false), virtual_key(VK_LEFT, true)]);
                        }
                        inputs
                    })
                    .collect();
                if !inputs.is_empty() && !send(&inputs, target) {
                    session.reset();
                    state.active.store(false, Ordering::Release);
                    continue;
                }
                if !reply.consumed {
                    let _ = send(
                        &[
                            virtual_key(key.vk as u16, false),
                            virtual_key(key.vk as u16, true),
                        ],
                        target,
                    );
                }
                let view = reply.view;
                state.composing.store(
                    view.as_ref().is_some_and(|v| !v.text.is_empty()),
                    Ordering::Release,
                );
                if let Some(view) = view.filter(|v| !v.text.is_empty()) {
                    if view.text.chars().count() > 128
                        || view
                            .typed
                            .as_ref()
                            .is_some_and(|raw| raw.chars().count() > 128)
                    {
                        session.reset();
                        state.active.store(false, Ordering::Release);
                        ShowWindow(overlay, SW_HIDE);
                        continue;
                    }
                    let prediction = !view.converting && view.candidates.len() <= 1;
                    let visible_candidates = if prediction {
                        &view.predictions
                    } else {
                        &view.candidates
                    };
                    let selected = if prediction {
                        view.selected_prediction
                    } else {
                        view.selected_index as i32
                    };
                    let candidates = visible_candidates
                        .iter()
                        .take(7)
                        .enumerate()
                        .map(|(i, text)| {
                            format!(
                                "{}{} {}",
                                if i as i32 == selected { ">" } else { "" },
                                i + 1,
                                text
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\r\n");
                    let display: Vec<u16> = format!(
                        "{}\r\n{}\r\n{}\r\nCtrl+Alt+J: 入力切替 / Esc: 取消",
                        view.typed.as_deref().unwrap_or(""),
                        view.text,
                        candidates
                    )
                    .encode_utf16()
                    .chain(Some(0))
                    .collect();
                    SetWindowTextW(overlay, display.as_ptr());
                    // Use the input field, rather than a fixed desktop corner,
                    // when Chromium exposes no Win32 caret window.
                    if let Some(element) = &focused {
                        if let Ok(rect) = element.CurrentBoundingRectangle() {
                            let mut caret = GUITHREADINFO {
                                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                                ..Default::default()
                            };
                            let thread =
                                GetWindowThreadProcessId(target as HWND, std::ptr::null_mut());
                            let mut point = POINT {
                                x: rect.left,
                                y: rect.bottom,
                            };
                            if GetGUIThreadInfo(thread, &mut caret) != 0
                                && !caret.hwndCaret.is_null()
                            {
                                point = POINT {
                                    x: caret.rcCaret.left,
                                    y: caret.rcCaret.bottom,
                                };
                                ClientToScreen(caret.hwndCaret, &mut point);
                            }
                            let mut work = RECT::default();
                            SystemParametersInfoW(
                                SPI_GETWORKAREA,
                                0,
                                (&mut work as *mut RECT).cast(),
                                0,
                            );
                            let width = 520.min((work.right - work.left).max(1));
                            let height = (88 + visible_candidates.len().min(7) as i32 * 24)
                                .min((work.bottom - work.top).max(1));
                            SetWindowPos(
                                overlay,
                                HWND_TOPMOST,
                                point
                                    .x
                                    .clamp(work.left, (work.right - width).max(work.left)),
                                point
                                    .y
                                    .clamp(work.top, (work.bottom - height).max(work.top)),
                                width,
                                height,
                                SWP_NOACTIVATE,
                            );
                        }
                    }
                    ShowWindow(overlay, SW_SHOWNOACTIVATE);
                } else {
                    ShowWindow(overlay, SW_HIDE);
                }
            }
            DestroyWindow(overlay);
            let _ = hook_thread.join();
            CoUninitialize();
        });
        match ready_rx.recv_timeout(Duration::from_secs(15)) {
            Ok(Ok(())) => Ok(Self {
                shared,
                worker: Some(worker),
                last_settings: None,
            }),
            result => {
                shared.exit.store(true, Ordering::Release);
                Err(format!("ポータブル入力の起動に失敗しました: {result:?}"))
            }
        }
    }
}
impl Drop for Keyboard {
    fn drop(&mut self) {
        self.shared.active.store(false, Ordering::Release);
        self.shared.exit.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_pattern_read_only_requires_an_explicit_boolean() {
        use windows::core::VARIANT;
        assert_eq!(read_only_bool(VARIANT::from(false)), Some(false));
        assert_eq!(read_only_bool(VARIANT::from(true)), Some(true));
        assert_eq!(read_only_bool(VARIANT::new()), None);
        assert_eq!(read_only_bool(VARIANT::from(0i32)), None);
        assert_eq!(read_only_bool(VARIANT::from("false")), None);
    }
    #[test]
    #[ignore = "interactive Windows acceptance; drive with tools/test-portable-input.py --unit-host"]
    fn external_input_host() {
        let directory = std::path::PathBuf::from(
            std::env::var_os("MELTYPE_RUST_DATA_DIR").expect("isolated test data"),
        );
        let keyboard = Keyboard::start(Store::new(directory.clone())).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(90);
        while !directory.join("stop-host").exists() && std::time::Instant::now() < deadline {
            let ready = directory.join("ready-host");
            if keyboard.shared.active.load(Ordering::Acquire)
                && keyboard.shared.ready.load(Ordering::Acquire)
            {
                std::fs::write(&ready, b"ready").unwrap();
            } else if ready.exists() {
                std::fs::remove_file(&ready).unwrap();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(keyboard);
        assert!(
            directory.join("stop-host").exists(),
            "acceptance driver timed out"
        );
    }
    #[test]
    fn idle_navigation_passes_and_injection_is_tagged_utf16() {
        assert!(captures(13, 0, false));
        assert!(captures(13, 0, true));
        assert!(captures(0x41, 'a' as i32, false));
        assert!(!captures(0x70, 0, true));
        let inputs = unicode("日😀");
        assert_eq!(inputs.len(), 6);
        for input in inputs {
            unsafe {
                assert_eq!(input.Anonymous.ki.dwExtraInfo, INJECTED_TAG);
                assert_eq!(input.Anonymous.ki.wVk, 0);
            }
        }
    }
}
