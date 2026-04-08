use crate::dlog;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use tauri::{AppHandle, Emitter, Manager};

/// Target key for double-tap: 0=Control, 1=Alt, 2=Shift
static DOUBLE_TAP_TARGET: AtomicU8 = AtomicU8::new(0);
/// When true, double-tap polling is disabled (combo mode active)
static DOUBLE_TAP_DISABLED: AtomicBool = AtomicBool::new(false);

pub fn set_hook_target_key(hotkey: &str) {
    let val = match hotkey {
        "Alt" => 1,
        "Shift" => 2,
        _ => 0,
    };
    DOUBLE_TAP_TARGET.store(val, Ordering::Relaxed);
}

pub struct HotkeyState {
    pub double_tap_disabled: AtomicBool,
}

impl HotkeyState {
    pub fn new() -> Self {
        Self {
            double_tap_disabled: AtomicBool::new(false),
        }
    }
}

/// Returns true if the hotkey string is a combination (contains "+")
pub fn is_combo_hotkey(hotkey: &str) -> bool {
    hotkey.contains('+')
}

/// Get the monitor handle (as isize) for the window's current position.
#[cfg(target_os = "windows")]
pub fn get_window_monitor(window: &tauri::WebviewWindow) -> isize {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{MonitorFromRect, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let hwnd = match window.hwnd() {
        Ok(h) => HWND(h.0),
        Err(_) => return 0,
    };
    unsafe {
        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);
        MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST).0 as isize
    }
}

pub fn toggle_launcher(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    toggle_launcher_win32(app);

    #[cfg(not(target_os = "windows"))]
    toggle_launcher_fallback(app);
}

#[cfg(not(target_os = "windows"))]
fn toggle_launcher_fallback(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.emit("launcher-show", ());
        }
    }
}

/// Win32 for detection, Tauri API for show/hide to keep framework state in sync.
#[cfg(target_os = "windows")]
fn toggle_launcher_win32(app: &AppHandle) {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        MonitorFromPoint, MonitorFromRect, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect, IsWindowVisible};

    let window = match app.get_webview_window("main") {
        Some(w) => w,
        None => return,
    };
    let hwnd = match window.hwnd() {
        Ok(h) => HWND(h.0),
        Err(_) => return,
    };

    // Suppress blur_hide during the operation
    let state = app.state::<crate::AppState>();
    let blur_was_enabled = state.blur_hide_enabled.lock().map(|v| *v).unwrap_or(true);
    if blur_was_enabled {
        if let Ok(mut v) = state.blur_hide_enabled.lock() {
            *v = false;
        }
    }

    unsafe {
        let visible = IsWindowVisible(hwnd).as_bool();
        let mut cursor = POINT::default();
        let _ = GetCursorPos(&mut cursor);
        let cursor_monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);

        dlog(&format!(
            "toggle: visible={} cursor=({},{})",
            visible, cursor.x, cursor.y
        ));

        // Check if the window was recently hidden by blur_hide (within 2 seconds).
        let recent_blur = state.last_blur_hide.lock().ok().and_then(|lb| {
            lb.and_then(|(time, monitor)| {
                if time.elapsed().as_millis() < 2000 {
                    Some(monitor)
                } else {
                    None
                }
            })
        });

        // Clear the blur record so it's consumed once
        if recent_blur.is_some() {
            if let Ok(mut lb) = state.last_blur_hide.lock() {
                *lb = None;
            }
        }

        if visible {
            let mut win_rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut win_rect);
            let window_monitor = MonitorFromRect(&win_rect, MONITOR_DEFAULTTONEAREST);
            let same_monitor = cursor_monitor == window_monitor;

            dlog(&format!(
                "toggle: win=({},{},{},{}) same_mon={}",
                win_rect.left, win_rect.top, win_rect.right, win_rect.bottom, same_monitor
            ));

            if same_monitor {
                dlog("toggle: -> HIDE");
                let _ = window.hide();
                restore_blur_hide_sync(app, blur_was_enabled);
            } else {
                dlog("toggle: -> MOVE to other monitor");
                let _ = window.hide();
                move_and_show(hwnd, cursor, app);
                let _ = window.show();
                force_foreground_win32(hwnd);
                let _ = window.set_focus();
                let _ = window.emit("launcher-show", ());
                restore_blur_hide(app, blur_was_enabled);
            }
        } else if let Some(blur_monitor) = recent_blur {
            let same_monitor = cursor_monitor.0 as isize == blur_monitor;
            dlog(&format!(
                "toggle: recently blurred, same_mon={}",
                same_monitor
            ));

            if same_monitor {
                dlog("toggle: -> STAY HIDDEN (blur grace, same monitor)");
                restore_blur_hide_sync(app, blur_was_enabled);
            } else {
                dlog("toggle: -> SHOW on new monitor (blur grace)");
                move_and_show(hwnd, cursor, app);
                let _ = window.show();
                force_foreground_win32(hwnd);
                let _ = window.set_focus();
                let _ = window.emit("launcher-show", ());
                restore_blur_hide(app, blur_was_enabled);
            }
        } else {
            dlog("toggle: -> SHOW");
            move_and_show(hwnd, cursor, app);
            let _ = window.show();
            force_foreground_win32(hwnd);
            let _ = window.set_focus();
            let _ = window.emit("launcher-show", ());
            restore_blur_hide(app, blur_was_enabled);
        }
    }
}

/// Position the window centered on the cursor's monitor (DPI-aware).
#[cfg(target_os = "windows")]
unsafe fn move_and_show(
    hwnd: windows::Win32::Foundation::HWND,
    cursor: windows::Win32::Foundation::POINT,
    app: &tauri::AppHandle,
) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE,
    };

    let hmonitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(hmonitor, &mut mi).as_bool() {
        return;
    }

    let rc: RECT = mi.rcWork;
    let mon_w = (rc.right - rc.left) as f64;
    let mon_h = (rc.bottom - rc.top) as f64;

    // Get DPI scale for the target monitor
    let mut dpi_x: u32 = 96;
    let mut dpi_y: u32 = 96;
    let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
    let scale = dpi_x as f64 / 96.0;

    // Read stored logical size and convert to physical for this monitor
    let (lw, lh) = app
        .state::<crate::AppState>()
        .window_logical_size
        .lock()
        .map(|s| *s)
        .unwrap_or((680, 460));
    let phys_w = lw as f64 * scale;
    let phys_h = lh as f64 * scale;

    let x = rc.left as f64 + (mon_w - phys_w) / 2.0;
    let y = rc.top as f64 + (mon_h - phys_h) / 3.0;

    dlog(&format!(
        "move_and_show: dpi={} scale={:.2} logical={}x{} phys={:.0}x{:.0} pos=({:.0},{:.0})",
        dpi_x, scale, lw, lh, phys_w, phys_h, x, y
    ));

    let _ = SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        x as i32,
        y as i32,
        0,
        0,
        SWP_NOSIZE | SWP_NOACTIVATE,
    );
}

/// Force foreground using Alt trick + AttachThreadInput.
#[cfg(target_os = "windows")]
unsafe fn force_foreground_win32(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
        VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };

    let foreground = GetForegroundWindow();
    let fg_thread = GetWindowThreadProcessId(foreground, None);
    let our_thread = GetCurrentThreadId();

    let alt_down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0x12),
                dwFlags: KEYEVENTF_EXTENDEDKEY,
                ..Default::default()
            },
        },
    };
    let alt_up = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0x12),
                dwFlags: KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP,
                ..Default::default()
            },
        },
    };
    let _ = SendInput(&[alt_down, alt_up], std::mem::size_of::<INPUT>() as i32);

    if fg_thread != our_thread && fg_thread != 0 {
        let _ = AttachThreadInput(our_thread, fg_thread, true);
        let _ = SetForegroundWindow(hwnd);
        let _ = AttachThreadInput(our_thread, fg_thread, false);
    } else {
        let _ = SetForegroundWindow(hwnd);
    }
}

fn restore_blur_hide(app: &AppHandle, was_enabled: bool) {
    if !was_enabled {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(800));
        dlog("restore_blur_hide: re-enabling");
        if let Ok(mut v) = app.state::<crate::AppState>().blur_hide_enabled.lock() {
            *v = true;
        }
    });
}

fn restore_blur_hide_sync(app: &AppHandle, was_enabled: bool) {
    if !was_enabled {
        return;
    }
    if let Ok(mut v) = app.state::<crate::AppState>().blur_hide_enabled.lock() {
        *v = true;
    }
}

/// Register a global shortcut combination (e.g. "Ctrl+Space").
pub fn register_combo_shortcut(app: &AppHandle, shortcut_str: &str) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let shortcut: tauri_plugin_global_shortcut::Shortcut = shortcut_str
        .parse()
        .map_err(|e| format!("Invalid shortcut '{}': {}", shortcut_str, e))?;

    let app_handle = app.clone();
    gs.on_shortcut(shortcut, move |_app, _shortcut, event| {
        if let tauri_plugin_global_shortcut::ShortcutState::Pressed = event.state {
            let h = app_handle.clone();
            std::thread::spawn(move || toggle_launcher(&h));
        }
    })
    .map_err(|e| format!("Failed to register shortcut: {}", e))?;

    Ok(())
}

/// Unregister all combo shortcuts (when switching back to double-tap mode).
pub fn unregister_combo_shortcuts(app: &AppHandle) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let _ = app.global_shortcut().unregister_all();
}

/// Apply hotkey setting: set up the right mode (double-tap or combo).
pub fn apply_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let hs = app.state::<HotkeyState>();
    if is_combo_hotkey(hotkey) {
        DOUBLE_TAP_DISABLED.store(true, Ordering::Relaxed);
        hs.double_tap_disabled.store(true, Ordering::Relaxed);
        register_combo_shortcut(app, hotkey)?;
    } else {
        set_hook_target_key(hotkey);
        DOUBLE_TAP_DISABLED.store(false, Ordering::Relaxed);
        hs.double_tap_disabled.store(false, Ordering::Relaxed);
        unregister_combo_shortcuts(app);
    }
    Ok(())
}

// ── Double-tap detection via GetAsyncKeyState polling ──
//
// WH_KEYBOARD_LL hooks can be silently removed by Windows when the callback
// takes too long or the message pump stalls (e.g. WebView2 interference).
// Polling GetAsyncKeyState avoids this entirely — no message pump dependency.

#[cfg(target_os = "windows")]
pub fn start_keyboard_hook(app_handle: AppHandle) {
    std::thread::spawn(move || {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
        };

        dlog("poll: key polling thread started");

        let mut was_pressed = false;
        let mut last_release_ms: u64 = 0;

        loop {
            std::thread::sleep(std::time::Duration::from_millis(10));

            if DOUBLE_TAP_DISABLED.load(Ordering::Relaxed) {
                was_pressed = false;
                continue;
            }

            let target = DOUBLE_TAP_TARGET.load(Ordering::Relaxed);
            let (vk1, vk2) = match target {
                1 => (VK_LMENU, VK_RMENU),
                2 => (VK_LSHIFT, VK_RSHIFT),
                _ => (VK_LCONTROL, VK_RCONTROL),
            };

            // GetAsyncKeyState: high bit set = currently pressed
            let pressed = unsafe {
                (GetAsyncKeyState(vk1.0 as i32) as u16 & 0x8000) != 0
                    || (GetAsyncKeyState(vk2.0 as i32) as u16 & 0x8000) != 0
            };

            if was_pressed && !pressed {
                // Key was just released — check for double-tap
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);

                if last_release_ms > 0 && now_ms - last_release_ms < 400 {
                    // Double-tap detected!
                    last_release_ms = 0;
                    let app_clone = app_handle.clone();
                    std::thread::spawn(move || {
                        dlog("poll: double-tap detected");
                        toggle_launcher(&app_clone);
                    });
                } else {
                    last_release_ms = now_ms;
                }
            }

            was_pressed = pressed;
        }
    });
}

#[cfg(not(target_os = "windows"))]
pub fn start_keyboard_hook(_app_handle: AppHandle) {
    log::warn!("Keyboard hook not implemented for this platform.");
}
