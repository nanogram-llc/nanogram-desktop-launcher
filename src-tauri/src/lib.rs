mod commands;
mod db;
mod hotkey;
mod icons;
mod scanner;
mod tray;

use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

/// Global debug log flag + path (set once at startup)
static DEBUG_LOG_ENABLED: AtomicBool = AtomicBool::new(false);
static mut DEBUG_LOG_PATH: Option<String> = None;

/// Initialize debug logging
pub fn init_debug_log(settings_path: &str) {
    let settings_content = std::fs::read_to_string(settings_path).unwrap_or_default();
    let enabled = serde_json::from_str::<serde_json::Value>(&settings_content)
        .ok()
        .and_then(|v| v.get("debug_log").and_then(|d| d.as_bool()))
        .unwrap_or(false);

    DEBUG_LOG_ENABLED.store(enabled, Ordering::Relaxed);
    if enabled {
        let log_path = std::path::Path::new(settings_path)
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("debug.log")
            .to_string_lossy()
            .to_string();
        // Clear previous log
        let _ = std::fs::write(&log_path, "");
        unsafe {
            DEBUG_LOG_PATH = Some(log_path);
        }
    }
}

/// Write a debug log line (timestamped, appended to debug.log next to exe)
pub fn dlog(msg: &str) {
    if !DEBUG_LOG_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let path = unsafe {
        match &raw const DEBUG_LOG_PATH {
            p => match &*p {
                Some(s) => s.clone(),
                None => return,
            },
        }
    };
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let now = chrono::Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(f, "[{}] {}", now, msg);
    }
}

pub struct AppState {
    pub db: Mutex<Connection>,
    pub icon_cache_dir: String,
    /// In-memory cache: icon_cache_path -> base64 data URI
    pub icon_mem_cache: Mutex<HashMap<String, String>>,
    /// Path to settings.json (next to the exe)
    pub settings_path: String,
    /// When true, window won't hide on blur (used during settings)
    pub blur_hide_enabled: Mutex<bool>,
    /// Current hotkey setting (e.g. "Control", "Alt", "Shift")
    pub current_hotkey: Mutex<String>,
    /// Timestamp + monitor of last blur_hide (for double-tap grace period)
    pub last_blur_hide: Mutex<Option<(std::time::Instant, isize)>>,
    /// Logical window size (w, h) — updated by resize_window, used for DPI-aware positioning
    pub window_logical_size: Mutex<(u32, u32)>,
    /// Recently launched exe names with timestamp — prevents duplicate launches during startup delay
    pub recent_launches: Mutex<HashMap<String, std::time::Instant>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // All data is stored next to the EXE so the app is fully portable.
            let exe_dir = std::env::current_exe()
                .unwrap_or_default()
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("."));

            let icon_cache_dir = exe_dir.join("icons");
            std::fs::create_dir_all(&icon_cache_dir).ok();

            let db_path = exe_dir.join("launcher.db");

            // One-time migration: copy DB from old %APPDATA% location if new path is empty.
            if !db_path.exists() {
                if let Ok(app_data_dir) = app.path().app_data_dir() {
                    let candidates = [
                        app_data_dir.join("launcher.db"),
                        app_data_dir.join("lancher.db"),
                    ];
                    for old in &candidates {
                        if old.exists() {
                            let _ = std::fs::copy(old, &db_path);
                            break;
                        }
                    }
                    // Migrate icons if new icons dir is still empty.
                    let old_icons = app_data_dir.join("icons");
                    if old_icons.is_dir() {
                        if let Ok(entries) = std::fs::read_dir(&old_icons) {
                            for entry in entries.flatten() {
                                let dest = icon_cache_dir.join(entry.file_name());
                                if !dest.exists() {
                                    let _ = std::fs::copy(entry.path(), dest);
                                }
                            }
                        }
                    }
                }
            }

            let conn = db::init_db(&db_path).expect("Failed to initialize database");

            // Settings JSON path: next to the executable
            let settings_path = std::env::current_exe()
                .unwrap_or_default()
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("settings.json")
                .to_string_lossy()
                .to_string();

            // Initialize debug logging
            init_debug_log(&settings_path);
            dlog("=== App starting ===");

            // Load settings once for use during startup
            let settings_content = std::fs::read_to_string(&settings_path).unwrap_or_default();
            let settings_json: serde_json::Value =
                serde_json::from_str(&settings_content).unwrap_or_default();

            // Load initial hotkey from settings
            let initial_hotkey = settings_json
                .get("hotkey")
                .and_then(|h| h.as_str().map(String::from))
                .unwrap_or_else(|| "Control".to_string());

            // Sync autostart: if enabled, re-register with the current EXE path
            // (important for portable apps where the path may change across machines).
            let autostart_enabled = settings_json
                .get("autostart")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            commands::settings::apply_autostart(app.handle(), autostart_enabled);

            app.manage(AppState {
                db: Mutex::new(conn),
                icon_cache_dir: icon_cache_dir.to_string_lossy().to_string(),
                icon_mem_cache: Mutex::new(HashMap::new()),
                settings_path,
                blur_hide_enabled: Mutex::new(true),
                current_hotkey: Mutex::new(initial_hotkey),
                last_blur_hide: Mutex::new(None),
                window_logical_size: Mutex::new((680, 460)),
                recent_launches: Mutex::new(HashMap::new()),
            });

            // Setup hotkey state
            let hotkey_state = hotkey::HotkeyState::new();
            app.manage(hotkey_state);

            // Start low-level keyboard hook for double-tap detection
            hotkey::start_keyboard_hook(app.handle().clone());

            // Apply initial hotkey mode (combo or double-tap)
            let initial_hk = app
                .state::<AppState>()
                .current_hotkey
                .lock()
                .map(|h| h.clone())
                .unwrap_or_else(|_| "Control".to_string());
            if let Err(e) = hotkey::apply_hotkey(app.handle(), &initial_hk) {
                eprintln!("Failed to apply initial hotkey: {}", e);
            }

            // Apply initial window size from settings
            {
                let settings_content = std::fs::read_to_string(&app.state::<AppState>().settings_path).unwrap_or_default();
                let settings: db::models::Settings = serde_json::from_str(&settings_content).unwrap_or_default();
                let section_count = settings.sections.iter().filter(|s| s.visible).count() as u32;
                let pinned_visible = settings.sections.iter().any(|s| s.id == "pinned" && s.visible);
                let pinned_rows = if pinned_visible { settings.pinned_rows } else { 0 };
                let extra_rows = section_count - if pinned_visible { 1 } else { 0 };
                let total_rows = extra_rows + pinned_rows;
                let (w, h) = commands::window::calc_window_size(settings.pinned_columns, total_rows, section_count);
                if let Ok(mut ls) = app.state::<AppState>().window_logical_size.lock() {
                    *ls = (w, h);
                }
                if let Some(ref window) = app.get_webview_window("main") {
                    let size = tauri::LogicalSize::new(w as f64, h as f64);
                    let _ = window.set_size(tauri::Size::Logical(size));
                }
            }

            // Apply window vibrancy
            if let Some(ref window) = app.get_webview_window("main") {
                eprintln!("[setup] Window 'main' found OK");
                #[cfg(target_os = "windows")]
                {
                    use window_vibrancy::{apply_acrylic, apply_mica};
                    let vibrancy_ok = if apply_mica(&window, Some(true)).is_ok() {
                        dlog("vibrancy: Mica applied");
                        true
                    } else {
                        dlog("vibrancy: Mica not supported, trying Acrylic...");
                        if apply_acrylic(&window, Some((18, 18, 18, 200))).is_ok() {
                            dlog("vibrancy: Acrylic applied");
                            true
                        } else {
                            dlog("vibrancy: both Mica and Acrylic failed");
                            false
                        }
                    };
                    // When vibrancy is active, make the webview background transparent
                    // so the native blur effect shows through.
                    // When vibrancy fails, keep the CSS fallback background.
                    if vibrancy_ok {
                        let _ = window
                            .eval("document.documentElement.style.backgroundColor='transparent'");
                    }
                }

                #[cfg(target_os = "macos")]
                {
                    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                    let _ = apply_vibrancy(
                        &window,
                        NSVisualEffectMaterial::UnderWindowBackground,
                        None,
                        None,
                    );
                }

                // Hide on blur (focus lost) — only when blur_hide is enabled
                let win = window.clone();
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    match event {
                        tauri::WindowEvent::Focused(focused) => {
                            dlog(&format!("window: Focused({})", focused));
                            if !focused {
                                let state = app_handle.state::<AppState>();
                                let enabled =
                                    state.blur_hide_enabled.lock().map(|v| *v).unwrap_or(true);
                                if enabled {
                                    // Record which monitor the window was on before hiding
                                    #[cfg(target_os = "windows")]
                                    {
                                        let monitor_id = hotkey::get_window_monitor(&win);
                                        if let Ok(mut lb) = state.last_blur_hide.lock() {
                                            *lb = Some((std::time::Instant::now(), monitor_id));
                                        }
                                        dlog(&format!(
                                            "window: blur_hide -> hiding (monitor={})",
                                            monitor_id
                                        ));
                                    }
                                    #[cfg(not(target_os = "windows"))]
                                    dlog("window: blur_hide -> hiding");
                                    let _ = win.hide();
                                } else {
                                    dlog("window: blur_hide disabled, skip hide");
                                }
                            }
                        }
                        _ => {}
                    }
                });
            }

            // Setup system tray
            tray::setup_tray(app.handle())?;

            // Sync registered_items from settings.json into DB
            if let Err(e) =
                commands::settings::sync_registered_items(app.state::<AppState>().inner())
            {
                eprintln!("Failed to sync registered items: {}", e);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::apps::get_apps,
            commands::apps::launch_app,
            commands::apps::delete_app,
            commands::apps::rename_app,
            commands::apps::set_tags,
            commands::apps::exclude_app,
            commands::apps::include_app,
            commands::apps::get_excluded_apps,
            commands::apps::bulk_exclude_apps,
            commands::apps::bulk_include_apps,
            commands::apps::register_files,
            commands::apps::register_items,
            commands::apps::get_items_by_type,
            commands::apps::get_excluded_items_by_type,
            commands::apps::open_item,
            commands::apps::launch_app_as_admin,
            commands::apps::open_folder_with,
            commands::apps::register_command,
            commands::apps::update_command,
            commands::apps::get_command_data,
            commands::apps::launch_command,
            commands::folders::add_folder,
            commands::folders::remove_folder,
            commands::folders::get_folders,
            commands::folders::rescan_folder,
            commands::folders::get_startmenu_paths,
            commands::icons::get_icon,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::open_settings_file,
            commands::window::set_blur_hide,
            commands::window::resize_window,
            commands::window::get_exe_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
