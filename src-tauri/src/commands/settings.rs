use crate::db::models::{AppEntry, Folder, Settings};
use crate::db::queries;
use crate::hotkey;
use crate::icons;
use crate::AppState;
use chrono::Utc;
use std::path::Path;
use tauri::State;
use tauri_plugin_autostart::ManagerExt;
use uuid::Uuid;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    load_settings_from_file(&state.settings_path)
}

#[tauri::command]
pub async fn update_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<(), String> {
    // Check if hotkey changed
    let hotkey_changed = {
        let current = state.current_hotkey.lock().map_err(|e| e.to_string())?;
        *current != settings.hotkey
    };

    // Update in-memory hotkey state
    if let Ok(mut current) = state.current_hotkey.lock() {
        *current = settings.hotkey.clone();
    }

    // Apply new hotkey mode if changed
    if hotkey_changed {
        hotkey::apply_hotkey(&app, &settings.hotkey)?;
    }

    // Apply autostart (Windows registry / macOS LaunchAgent)
    apply_autostart(&app, settings.autostart);

    save_settings_to_file(&state.settings_path, &settings)
}

/// Enable or disable OS-level autostart.
pub fn apply_autostart(app: &tauri::AppHandle, enable: bool) {
    let mgr = app.autolaunch();
    if enable {
        if let Err(e) = mgr.enable() {
            eprintln!("autostart enable failed: {}", e);
        }
    } else {
        if let Err(e) = mgr.disable() {
            eprintln!("autostart disable failed: {}", e);
        }
    }
}

#[tauri::command]
pub async fn open_settings_file(state: State<'_, AppState>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &state.settings_path])
            .spawn()
            .map_err(|e| format!("Failed to open settings file: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&state.settings_path)
            .spawn()
            .map_err(|e| format!("Failed to open settings file: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&state.settings_path)
            .spawn()
            .map_err(|e| format!("Failed to open settings file: {}", e))?;
    }
    Ok(())
}

fn load_settings_from_file(path: &str) -> Result<Settings, String> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse settings: {}", e))
        }
        Err(_) => {
            // File doesn't exist yet — return defaults and create the file
            let defaults = Settings::default();
            let _ = save_settings_to_file(path, &defaults);
            Ok(defaults)
        }
    }
}

fn save_settings_to_file(path: &str, settings: &Settings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    std::fs::write(path, json).map_err(|e| format!("Failed to write settings: {}", e))
}

/// Sync registered_items from settings.json into the database.
/// Called on startup and when settings are updated.
pub fn sync_registered_items(state: &AppState) -> Result<u32, String> {
    let settings = load_settings_from_file(&state.settings_path)?;
    if settings.registered_items.is_empty() {
        return Ok(0);
    }

    let conn = state.lock_db()?;
    let now = Utc::now().timestamp();
    let mut count = 0u32;

    for item in &settings.registered_items {
        let path_str = item.path();
        let item_type = item.item_type();
        let path = Path::new(path_str);

        // Validate existence
        match item_type {
            "folder" => {
                if !path.is_dir() {
                    continue;
                }
            }
            _ => {
                if !path.is_file() {
                    continue;
                }
            }
        }

        // Skip if already registered
        if queries::is_exe_path_registered(&conn, path_str).unwrap_or(true) {
            continue;
        }

        // Determine virtual folder
        let (folder_id, folder_label) = match item_type {
            "file" => ("settings-file-registered", "(Registered Files)"),
            "folder" => ("settings-folder-registered", "(Registered Folders)"),
            _ => ("settings-registered", "(Registered Apps)"),
        };

        // Ensure virtual folder exists
        let folder_exists = queries::get_folder_by_id(&conn, folder_id)
            .map_err(|e| e.to_string())?
            .is_some();
        if !folder_exists {
            let folder = Folder {
                id: folder_id.to_string(),
                path: folder_label.to_string(),
                added_at: now,
                last_scanned: Some(now),
            };
            queries::insert_folder(&conn, &folder).map_err(|e| e.to_string())?;
        }

        // Determine name
        let name = item.name().map(|s| s.to_string()).unwrap_or_else(|| {
            if item_type == "app" {
                // Try to extract from PE metadata or lnk
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                match ext.as_str() {
                    "lnk" => crate::scanner::resolve_single_lnk(path)
                        .map(|a| a.name)
                        .unwrap_or_else(|| {
                            path.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string()
                        }),
                    _ => crate::scanner::extract_name_from_exe(path).unwrap_or_else(|| {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string()
                    }),
                }
            } else {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path_str)
                    .to_string()
            }
        });

        let id = Uuid::new_v4().to_string();

        // Extract icon
        let icon_cache_path = if item_type == "app" {
            icons::extract_and_cache_icon(path_str, Path::new(&state.icon_cache_dir), &id)
        } else {
            icons::extract_file_icon_and_cache(path_str, Path::new(&state.icon_cache_dir), &id)
        };

        let entry = AppEntry {
            id,
            name,
            exe_path: path_str.to_string(),
            folder_id: folder_id.to_string(),
            icon_cache_path,
            tags: item.tags(),
            launch_count: 0,
            last_launched: None,
            is_excluded: false,
            is_renamed: false,
            created_at: now,
            item_type: item_type.to_string(),
            icon_base64: None,
        };

        if queries::insert_app(&conn, &entry).is_ok() {
            count += 1;
        }
    }

    Ok(count)
}

/// Get the current excluded_patterns from settings.
pub fn get_excluded_patterns(state: &AppState) -> Vec<String> {
    load_settings_from_file(&state.settings_path)
        .map(|s| s.excluded_patterns)
        .unwrap_or_default()
}
