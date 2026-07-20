mod commands;
mod db;
mod hotkey;
mod icons;
mod scanner;
mod tray;

use rusqlite::Connection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

// ── Runtime/source bootstrap ──
// The launcher can be installed in a cloud-synced folder (Dropbox,
// OneDrive, …). To keep the EXE file unlocked for sync, the *source* EXE
// just copies itself to a local "runtime" path on first launch, spawns it,
// and exits. The runtime process is what holds the OS file lock — and it
// lives in %LOCALAPPDATA% which the cloud doesn't sync.

const RUNTIME_SUBDIR: &str = "nanogramLauncher\\runtime";
const RUNTIME_EXE_NAME: &str = "nanogram desktop LauncheR.exe";
/// Earlier versions used a different runtime EXE name; bootstrap cleans
/// these up so users don't end up with multiple stale binaries.
const LEGACY_RUNTIME_EXE_NAMES: &[&str] = &["nanogramLauncher.exe"];
const SOURCE_MARKER_FILE: &str = "source.txt";

#[derive(Debug, Clone)]
enum RuntimeState {
    /// The currently running EXE is the source copy (e.g. in a Dropbox
    /// folder). It should bootstrap to the runtime location and exit.
    Source { current_exe: PathBuf },
    /// The currently running EXE is the runtime copy in %LOCALAPPDATA%.
    Runtime {
        source_exe: PathBuf,
        source_dir: PathBuf,
    },
    /// Couldn't determine — fall back to running normally next to the EXE.
    Unknown { current_dir: PathBuf },
}

fn runtime_exe_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(base)
            .join(RUNTIME_SUBDIR)
            .join(RUNTIME_EXE_NAME),
    )
}

/// Local (non-cloud-synced) directory used for the evacuated DB working copy,
/// its merge-base snapshot, and the evacuation marker. Falls back to the
/// system temp dir if LOCALAPPDATA is unavailable.
fn local_runtime_dir() -> PathBuf {
    match std::env::var_os("LOCALAPPDATA") {
        Some(base) => PathBuf::from(base).join(RUNTIME_SUBDIR),
        None => std::env::temp_dir().join("nanogramLauncher"),
    }
}

fn detect_runtime_state() -> RuntimeState {
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => {
            return RuntimeState::Unknown {
                current_dir: PathBuf::from("."),
            };
        }
    };
    let parent = match current_exe.parent() {
        Some(p) => p.to_path_buf(),
        None => {
            return RuntimeState::Unknown {
                current_dir: PathBuf::from("."),
            };
        }
    };

    // The presence of source.txt next to us is the single source of truth
    // for "I am the runtime copy". Path comparisons are unreliable across
    // case differences and short-name aliases on Windows.
    let marker = parent.join(SOURCE_MARKER_FILE);
    if marker.is_file() {
        if let Ok(s) = std::fs::read_to_string(&marker) {
            let exe_path = PathBuf::from(s.trim());
            if let Some(dir) = exe_path.parent() {
                if dir.is_dir() {
                    return RuntimeState::Runtime {
                        source_exe: exe_path.clone(),
                        source_dir: dir.to_path_buf(),
                    };
                }
            }
        }
        // Stale marker — treat as unknown so we still boot.
        return RuntimeState::Unknown { current_dir: parent };
    }

    RuntimeState::Source { current_exe }
}

fn data_dir_for(state: &RuntimeState) -> PathBuf {
    match state {
        RuntimeState::Runtime { source_dir, .. } => source_dir.clone(),
        RuntimeState::Source { current_exe } => current_exe
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")),
        RuntimeState::Unknown { current_dir } => current_dir.clone(),
    }
}

/// Bootstrap from source location to the local runtime location.
/// Returns true if the runtime was successfully spawned (caller should exit).
fn bootstrap_to_runtime(current_exe: &Path) -> bool {
    let runtime_exe = match runtime_exe_path() {
        Some(p) => p,
        None => return false,
    };
    let runtime_dir = match runtime_exe.parent() {
        Some(p) => p.to_path_buf(),
        None => return false,
    };
    if std::fs::create_dir_all(&runtime_dir).is_err() {
        return false;
    }

    // Copy may fail if a previous runtime instance is still exiting; retry
    // for up to 30 seconds before giving up.
    let mut copied = false;
    for _ in 0..60 {
        if std::fs::copy(current_exe, &runtime_exe).is_ok() {
            copied = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    if !copied {
        return false;
    }

    // Persist the full source EXE path. We store the EXE path (not just
    // its directory) so the update watcher can still find it even if the
    // user has renamed the launcher binary.
    let _ = std::fs::write(
        runtime_dir.join(SOURCE_MARKER_FILE),
        current_exe.to_string_lossy().as_bytes(),
    );

    // Remove any leftover runtime binaries from previous releases so
    // the user doesn't accumulate stale EXEs in %LOCALAPPDATA%.
    for legacy in LEGACY_RUNTIME_EXE_NAMES {
        let legacy_path = runtime_dir.join(legacy);
        if legacy_path != runtime_exe && legacy_path.exists() {
            let _ = std::fs::remove_file(&legacy_path);
        }
    }

    std::process::Command::new(&runtime_exe).spawn().is_ok()
}

/// Watcher: if the source EXE (in the cloud-synced folder) gets updated to
/// a newer mtime than ours, hand off to it and exit so it can re-bootstrap
/// the runtime with the new binary.
fn spawn_update_watcher(source_exe: PathBuf) {
    let our_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let our_mtime = match std::fs::metadata(&our_exe).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return,
    };

    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(60));

        if !source_exe.is_file() {
            continue;
        }
        let src_mtime = match std::fs::metadata(&source_exe).and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if src_mtime <= our_mtime {
            continue;
        }

        // Wait a short grace period in case Dropbox is mid-write.
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Re-check that the file is still there and still newer.
        let src_mtime2 = match std::fs::metadata(&source_exe).and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if src_mtime2 <= our_mtime {
            continue;
        }

        if std::process::Command::new(&source_exe).spawn().is_ok() {
            // Brief pause so the spawned process gets past argv parsing
            // before our exit releases its parent handle and kills it on
            // some Windows configurations.
            std::thread::sleep(std::time::Duration::from_millis(500));
            std::process::exit(0);
        }
        return;
    });
}

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
    /// SQLite connection — closed when the launcher is hidden so Dropbox can
    /// sync `launcher.db`, lazy-reopened on next access via `lock_db()`.
    pub db: Mutex<Option<Connection>>,
    /// Canonical DB in the cloud-synced source folder (next to the original EXE).
    pub dropbox_db_path: PathBuf,
    /// Local working copy in %LOCALAPPDATA%, used while the DB is evacuated so
    /// Dropbox can finish syncing without lock contention.
    pub local_db_path: PathBuf,
    /// Snapshot of the Dropbox DB taken at evacuation time — the common
    /// ancestor for a 3-way merge.
    pub evac_snapshot_path: PathBuf,
    /// Marker file whose existence means "currently evacuated". Machine-local
    /// (not stored in the shared settings.json) so multi-PC stays correct.
    pub evac_marker_path: PathBuf,
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

/// Guard returned by `AppState::lock_db()`. Derefs to `&Connection` so existing
/// `&conn` call sites work unchanged.
pub struct DbGuard<'a> {
    inner: MutexGuard<'a, Option<Connection>>,
}

impl<'a> std::ops::Deref for DbGuard<'a> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.inner
            .as_ref()
            .expect("DB connection should be open after lock_db()")
    }
}

impl AppState {
    /// True if the DB is currently evacuated to the local working copy.
    pub fn is_evacuated(&self) -> bool {
        self.evac_marker_path.exists()
    }

    /// The DB path currently in effect: the local copy while evacuated,
    /// otherwise the canonical Dropbox copy.
    pub fn active_db_path(&self) -> PathBuf {
        if self.is_evacuated() {
            self.local_db_path.clone()
        } else {
            self.dropbox_db_path.clone()
        }
    }

    /// Lock the DB, opening lazily if it was closed for Dropbox sync.
    pub fn lock_db(&self) -> Result<DbGuard<'_>, String> {
        let mut guard = self.db.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            let conn = crate::db::init_db(&self.active_db_path()).map_err(|e| e.to_string())?;
            *guard = Some(conn);
        }
        Ok(DbGuard { inner: guard })
    }

    /// Close the DB connection so Dropbox can sync `launcher.db`. Next
    /// `lock_db()` call will reopen automatically.
    pub fn close_db(&self) {
        if let Ok(mut guard) = self.db.lock() {
            *guard = None;
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = detect_runtime_state();

    // If we're the source copy, hand off to the runtime location and exit
    // so the source EXE file isn't held open against cloud sync.
    if let RuntimeState::Source { ref current_exe } = state {
        if bootstrap_to_runtime(current_exe) {
            std::process::exit(0);
        }
        // Bootstrap failed (couldn't write to %LOCALAPPDATA% etc.) — fall
        // through and run from the source path as a degraded fallback.
    }

    let data_dir = data_dir_for(&state);
    let setup_data_dir = data_dir.clone();
    let watcher_source_exe = match &state {
        RuntimeState::Runtime { source_exe, .. } => Some(source_exe.clone()),
        _ => None,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(move |app| {
            // Data lives in the source folder (next to the original EXE).
            // Even when we're running from %LOCALAPPDATA%, we look back at
            // the cloud-synced folder for DB / settings / icons.
            let exe_dir = setup_data_dir.clone();

            let icon_cache_dir = exe_dir.join("icons");
            std::fs::create_dir_all(&icon_cache_dir).ok();

            let dropbox_db_path = exe_dir.join("launcher.db");

            // One-time migration: copy DB from old %APPDATA% location if new path is empty.
            if !dropbox_db_path.exists() {
                if let Ok(app_data_dir) = app.path().app_data_dir() {
                    let candidates = [
                        app_data_dir.join("launcher.db"),
                        app_data_dir.join("lancher.db"),
                    ];
                    for old in &candidates {
                        if old.exists() {
                            let _ = std::fs::copy(old, &dropbox_db_path);
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

            // Evacuation paths live in %LOCALAPPDATA% (not cloud-synced).
            let local_dir = local_runtime_dir();
            let local_db_path = local_dir.join("launcher.db");
            let evac_snapshot_path = local_dir.join("launcher.db.base");
            let evac_marker_path = local_dir.join("evac.marker");

            // Degraded fallback: marker present but the local working copy is
            // gone (e.g. %LOCALAPPDATA% cleared) — clear the marker and use the
            // Dropbox copy so we still boot.
            if evac_marker_path.exists() && !local_db_path.exists() {
                let _ = std::fs::remove_file(&evac_marker_path);
            }

            // While evacuated, operate on the local working copy; otherwise the
            // canonical Dropbox copy.
            let active_db_path = if evac_marker_path.exists() {
                local_db_path.clone()
            } else {
                dropbox_db_path.clone()
            };

            let conn = db::init_db(&active_db_path).expect("Failed to initialize database");
            // Run any startup queries while we still have the eager connection,
            // then drop it so the file isn't held open during idle.

            // Settings JSON path: next to the executable
            // settings.json lives in the data directory (the source folder
            // when running from the runtime copy), not next to the running
            // EXE — so settings, pinned apps, etc. survive across the
            // bootstrap into %LOCALAPPDATA%.
            let settings_path = exe_dir.join("settings.json").to_string_lossy().to_string();

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
                db: Mutex::new(Some(conn)),
                dropbox_db_path: dropbox_db_path.clone(),
                local_db_path,
                evac_snapshot_path,
                evac_marker_path,
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
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let w_phys = (w as f64 * scale).ceil() as u32;
                    let h_phys = (h as f64 * scale).ceil() as u32;
                    let size = tauri::PhysicalSize::new(w_phys, h_phys);
                    let _ = window.set_size(tauri::Size::Physical(size));
                }
            }

            // Apply window vibrancy
            if let Some(ref window) = app.get_webview_window("main") {
                eprintln!("[setup] Window 'main' found OK");
                #[cfg(target_os = "windows")]
                {
                    use window_vibrancy::{apply_acrylic, apply_mica};
                    // Determine dark/light from settings (fallback: system preference)
                    let is_dark = {
                        let settings_content = std::fs::read_to_string(&app.state::<AppState>().settings_path).unwrap_or_default();
                        let settings: crate::db::models::Settings =
                            serde_json::from_str(&settings_content).unwrap_or_default();
                        match settings.theme.as_str() {
                            "dark" => true,
                            "light" => false,
                            _ => {
                                // system: check Windows registry dark mode preference
                                use std::process::Command;
                                let out = Command::new("reg")
                                    .args(["query", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize", "/v", "AppsUseLightTheme"])
                                    .output();
                                match out {
                                    Ok(o) => {
                                        let s = String::from_utf8_lossy(&o.stdout);
                                        // AppsUseLightTheme = 0x0 → dark, 0x1 → light
                                        !s.contains("0x1")
                                    }
                                    Err(_) => true, // assume dark on failure
                                }
                            }
                        }
                    };
                    let acrylic_color = if is_dark { (18u8, 18u8, 18u8, 200u8) } else { (240u8, 240u8, 240u8, 200u8) };
                    let vibrancy_ok = if apply_mica(&window, Some(is_dark)).is_ok() {
                        dlog("vibrancy: Mica applied");
                        true
                    } else {
                        dlog("vibrancy: Mica not supported, trying Acrylic...");
                        if apply_acrylic(&window, Some(acrylic_color)).is_ok() {
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
                                    // Release the DB file handle so Dropbox can sync.
                                    state.close_db();
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

            // Background: re-extract icons for apps where the initial pass
            // failed (icon_cache_path is NULL). Uses the SHGetFileInfoW
            // fallback so most missing icons get filled in.
            let app_handle_for_icons = app.handle().clone();
            std::thread::spawn(move || {
                let state = app_handle_for_icons.state::<AppState>();
                if let Ok(n) = commands::icons::regenerate_missing_icons_impl(state.inner()) {
                    if n > 0 {
                        dlog(&format!("startup: regenerated {} missing icons", n));
                    }
                }
                // Release the file handle once we're done so Dropbox can sync.
                state.close_db();
            });

            // Window starts hidden — close the DB so Dropbox can sync until the
            // user opens the launcher. (The background thread above may
            // reopen briefly; it closes again when finished.)
            app.state::<AppState>().close_db();

            // Watch the source EXE for an updated build (synced in by the
            // cloud client). When detected, hand off to the source — it
            // will re-bootstrap a new runtime copy and we exit.
            if let Some(src_exe) = watcher_source_exe.clone() {
                spawn_update_watcher(src_exe);
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
            commands::apps::reveal_in_explorer,
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
            commands::icons::regenerate_missing_icons,
            commands::icons::regenerate_all_icons,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::open_settings_file,
            commands::evac::get_evac_status,
            commands::evac::evacuate,
            commands::evac::merge_evacuation,
            commands::evac::discard_evacuation,
            commands::window::set_blur_hide,
            commands::window::resize_window,
            commands::window::set_launcher_size,
            commands::window::get_exe_dir,
            commands::window::set_vibrancy_theme,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
