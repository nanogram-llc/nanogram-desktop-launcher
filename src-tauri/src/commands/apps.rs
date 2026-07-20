use crate::db::models::AppEntry;
use crate::db::queries;
use crate::icons;
use crate::scanner;
use crate::AppState;
use chrono::Utc;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn get_apps(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: u32,
    include_items: Option<bool>,
) -> Result<Vec<AppEntry>, String> {
    let mut apps = {
        let conn = state.lock_db()?;
        if include_items.unwrap_or(false) {
            queries::get_all_launchable(&conn).map_err(|e| e.to_string())?
        } else {
            queries::get_all_apps(&conn).map_err(|e| e.to_string())?
        }
    };

    let mut result = match query {
        Some(q) if !q.trim().is_empty() => {
            let q = q.trim().to_lowercase();
            let now = Utc::now().timestamp();

            let mut scored: Vec<(f64, AppEntry)> = apps
                .into_iter()
                .filter_map(|app| {
                    let score = rank_app(&app, &q, now);
                    if score > 0.0 {
                        Some((score, app))
                    } else {
                        None
                    }
                })
                .collect();

            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(limit as usize);
            scored.into_iter().map(|(_, app)| app).collect::<Vec<_>>()
        }
        _ => {
            apps.truncate(limit as usize);
            apps
        }
    };

    // Load icons only for returned results, with in-memory cache
    {
        let mut cache = state.icon_mem_cache.lock().map_err(|e| e.to_string())?;
        for app in &mut result {
            if let Some(ref path) = app.icon_cache_path {
                if let Some(cached) = cache.get(path) {
                    app.icon_base64 = Some(cached.clone());
                } else if let Some(b64) = icons::load_icon_as_base64(path) {
                    cache.insert(path.clone(), b64.clone());
                    app.icon_base64 = Some(b64);
                }
            }
        }
    }

    Ok(result)
}

fn rank_app(app: &AppEntry, query: &str, now: i64) -> f64 {
    let name_lower = app.name.to_lowercase();
    let mut score = 0.0;

    // Name matching
    if name_lower == query {
        score += 100.0;
    } else if name_lower.starts_with(query) {
        score += 80.0;
    } else if name_lower.contains(query) {
        score += 50.0;
    } else if is_subsequence(query, &name_lower) {
        score += 30.0;
    }

    // Tag matching
    for tag in &app.tags {
        if tag.to_lowercase().contains(query) {
            score += 40.0;
        }
    }

    // If no text match at all, skip
    if score == 0.0 {
        return 0.0;
    }

    // Usage frequency boost (logarithmic)
    score += (app.launch_count as f64 + 1.0).ln() * 10.0;

    // Recency boost (7-day decay)
    if let Some(last) = app.last_launched {
        let hours_ago = (now - last) as f64 / 3600.0;
        score += 20.0 * (-hours_ago / 168.0_f64).exp();
    }

    score
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut haystack_chars = haystack.chars();
    for nc in needle.chars() {
        loop {
            match haystack_chars.next() {
                Some(hc) if hc == nc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

#[tauri::command]
pub async fn launch_app(state: State<'_, AppState>, app_id: String) -> Result<(), String> {
    let exe_path = {
        let conn = state.lock_db()?;
        let now = Utc::now().timestamp();
        queries::increment_launch(&conn, &app_id, now).map_err(|e| e.to_string())?;
        queries::get_app_exe_path(&conn, &app_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "App not found".to_string())?
    };

    // Resolve the actual exe name (follow .lnk targets)
    #[cfg(target_os = "windows")]
    let resolved_exe_name = {
        let p = std::path::Path::new(&exe_path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext == "lnk" {
            // Try resolving the shortcut target to get its exe name
            crate::scanner::resolve_single_lnk(p)
                .and_then(|app| {
                    std::path::Path::new(&app.exe_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_lowercase())
                })
                .unwrap_or_else(|| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase()
                })
        } else {
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase()
        }
    };

    #[cfg(target_os = "windows")]
    if !resolved_exe_name.is_empty() {
        // Check 1: Was this exe launched very recently? (prevents rapid double-launch)
        {
            let mut recent = state.recent_launches.lock().map_err(|e| e.to_string())?;
            // Clean up entries older than 10 seconds
            recent.retain(|_, t| t.elapsed().as_secs() < 10);
            if let Some(launched_at) = recent.get(&resolved_exe_name) {
                if launched_at.elapsed().as_secs() < 5 {
                    crate::dlog(&format!(
                        "launch_app: '{}' launched {}ms ago, skipping duplicate",
                        resolved_exe_name,
                        launched_at.elapsed().as_millis()
                    ));
                    // Still try to bring existing window to front
                    bring_process_to_front(&resolved_exe_name);
                    return Ok(());
                }
            }
        }

        // Check 2: Is the process already running?
        if is_process_running(&resolved_exe_name) {
            crate::dlog(&format!(
                "launch_app: '{}' already running, bringing to front",
                resolved_exe_name
            ));
            bring_process_to_front(&resolved_exe_name);
            return Ok(());
        }

        // Record this launch to prevent rapid duplicates
        {
            let mut recent = state.recent_launches.lock().map_err(|e| e.to_string())?;
            recent.insert(resolved_exe_name.clone(), std::time::Instant::now());
        }
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &exe_path])
            .spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&exe_path)
            .spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new(&exe_path)
            .spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;
    }

    Ok(())
}

/// Check if any process with the given exe filename is running.
#[cfg(target_os = "windows")]
fn is_process_running(exe_name: &str) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return false,
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len())],
                );
                if name.to_lowercase() == exe_name {
                    let _ = CloseHandle(snapshot);
                    return true;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
        false
    }
}

/// Try to bring the first window of a running process to the foreground.
#[cfg(target_os = "windows")]
fn bring_process_to_front(exe_name: &str) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
        ShowWindow, SW_RESTORE,
    };
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};

    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return,
        };

        let mut target_pid: u32 = 0;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len())],
                );
                if name.to_lowercase() == exe_name {
                    target_pid = entry.th32ProcessID;
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);

        if target_pid == 0 {
            return;
        }

        // Find a visible window belonging to this process
        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let pid_ptr = lparam.0 as *mut u32;
            let target_pid = *pid_ptr;
            let mut wnd_pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut wnd_pid));
            if wnd_pid == target_pid && IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
                // Store 0 to signal we found it
                *pid_ptr = 0;
                return BOOL(0); // stop enumeration
            }
            BOOL(1) // continue
        }

        let mut pid = target_pid;
        let _ = EnumWindows(Some(enum_callback), LPARAM(&mut pid as *mut u32 as isize));
    }
}

#[tauri::command]
pub async fn delete_app(state: State<'_, AppState>, app_id: String) -> Result<(), String> {
    let conn = state.lock_db()?;
    queries::delete_app(&conn, &app_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_app(
    state: State<'_, AppState>,
    app_id: String,
    new_name: String,
) -> Result<(), String> {
    let conn = state.lock_db()?;
    queries::rename_app(&conn, &app_id, &new_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_tags(
    state: State<'_, AppState>,
    app_id: String,
    tags: Vec<String>,
) -> Result<(), String> {
    let conn = state.lock_db()?;
    queries::set_tags(&conn, &app_id, &tags).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn exclude_app(state: State<'_, AppState>, app_id: String) -> Result<(), String> {
    let conn = state.lock_db()?;
    queries::set_excluded(&conn, &app_id, true).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn include_app(state: State<'_, AppState>, app_id: String) -> Result<(), String> {
    let conn = state.lock_db()?;
    queries::set_excluded(&conn, &app_id, false).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_excluded_apps(state: State<'_, AppState>) -> Result<Vec<AppEntry>, String> {
    let mut result = {
        let conn = state.lock_db()?;
        queries::get_excluded_apps(&conn).map_err(|e| e.to_string())?
    };

    // Load icons
    {
        let mut cache = state.icon_mem_cache.lock().map_err(|e| e.to_string())?;
        for app in &mut result {
            if let Some(ref path) = app.icon_cache_path {
                if let Some(cached) = cache.get(path) {
                    app.icon_base64 = Some(cached.clone());
                } else if let Some(b64) = icons::load_icon_as_base64(path) {
                    cache.insert(path.clone(), b64.clone());
                    app.icon_base64 = Some(b64);
                }
            }
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn bulk_exclude_apps(
    state: State<'_, AppState>,
    app_ids: Vec<String>,
) -> Result<(), String> {
    let conn = state.lock_db()?;
    queries::bulk_set_excluded(&conn, &app_ids, true).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn bulk_include_apps(
    state: State<'_, AppState>,
    app_ids: Vec<String>,
) -> Result<(), String> {
    let conn = state.lock_db()?;
    queries::bulk_set_excluded(&conn, &app_ids, false).map_err(|e| e.to_string())
}

/// Ensure a virtual folder exists for a given id/label.
fn ensure_virtual_folder(
    conn: &rusqlite::Connection,
    folder_id: &str,
    label: &str,
    now: i64,
) -> Result<(), String> {
    let exists = queries::get_folder_by_id(conn, folder_id)
        .map_err(|e| e.to_string())?
        .is_some();
    if !exists {
        let folder = crate::db::models::Folder {
            id: folder_id.to_string(),
            path: label.to_string(),
            added_at: now,
            last_scanned: Some(now),
        };
        queries::insert_folder(conn, &folder).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Register individual files (exe/lnk) dropped by the user.
#[tauri::command]
pub async fn register_files(state: State<'_, AppState>, paths: Vec<String>) -> Result<u32, String> {
    let now = Utc::now().timestamp();
    let conn = state.lock_db()?;
    let mut count = 0u32;

    ensure_virtual_folder(&conn, "drop-registered", "(Dropped Apps)", now)?;

    for file_path in &paths {
        let path = Path::new(file_path);
        if !path.is_file() {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let resolved: Vec<scanner::ScannedApp> = match ext.as_str() {
            "exe" => {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let name = scanner::extract_name_from_exe(path).unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(file_name)
                        .to_string()
                });
                vec![scanner::ScannedApp {
                    name,
                    exe_path: file_path.clone(),
                }]
            }
            "lnk" => {
                if let Some(app) = scanner::resolve_single_lnk(path) {
                    vec![app]
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        for app in resolved {
            if queries::is_exe_path_registered(&conn, &app.exe_path).unwrap_or(true) {
                continue;
            }

            let id = Uuid::new_v4().to_string();
            let icon_cache_path =
                icons::extract_and_cache_icon(&app.exe_path, Path::new(&state.icon_cache_dir), &id);

            let entry = AppEntry {
                id,
                name: app.name,
                exe_path: app.exe_path,
                folder_id: "drop-registered".to_string(),
                icon_cache_path,
                tags: Vec::new(),
                launch_count: 0,
                last_launched: None,
                is_excluded: false,
                is_renamed: false,
                created_at: now,
                item_type: "app".to_string(),
                icon_base64: None,
            };

            if queries::insert_app(&conn, &entry).is_ok() {
                count += 1;
            }
        }
    }

    Ok(count)
}

/// Register dropped items: files (documents) and folders.
#[tauri::command]
pub async fn register_items(
    state: State<'_, AppState>,
    paths: Vec<String>,
    item_type: String,
) -> Result<u32, String> {
    let now = Utc::now().timestamp();
    let conn = state.lock_db()?;
    let mut count = 0u32;

    let folder_id = match item_type.as_str() {
        "file" => "file-registered",
        "folder" => "folder-registered",
        _ => return Err("Invalid item_type".to_string()),
    };
    let folder_label = match item_type.as_str() {
        "file" => "(Registered Files)",
        _ => "(Registered Folders)",
    };
    ensure_virtual_folder(&conn, folder_id, folder_label, now)?;

    // Load allowed extensions for file type
    let allowed_exts: Vec<String> = if item_type == "file" {
        let settings_content = std::fs::read_to_string(&state.settings_path).unwrap_or_default();
        let settings: crate::db::models::Settings =
            serde_json::from_str(&settings_content).unwrap_or_default();
        settings
            .file_extensions
            .split(',')
            .map(|e| e.trim().to_lowercase())
            .filter(|e| !e.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    for file_path in &paths {
        let path = Path::new(file_path);

        // Validate based on type
        if item_type == "folder" && !path.is_dir() {
            continue;
        }
        if item_type == "file" {
            if !path.is_file() {
                continue;
            }
            // Check extension
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !allowed_exts.is_empty() && !allowed_exts.contains(&ext) {
                continue;
            }
        }

        if queries::is_exe_path_registered(&conn, file_path).unwrap_or(true) {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path)
            .to_string();

        let id = Uuid::new_v4().to_string();
        let icon_cache_path =
            icons::extract_file_icon_and_cache(file_path, Path::new(&state.icon_cache_dir), &id);

        let entry = AppEntry {
            id,
            name,
            exe_path: file_path.clone(),
            folder_id: folder_id.to_string(),
            icon_cache_path,
            tags: Vec::new(),
            launch_count: 0,
            last_launched: None,
            is_excluded: false,
            is_renamed: false,
            created_at: now,
            item_type: item_type.clone(),
            icon_base64: None,
        };

        if queries::insert_app(&conn, &entry).is_ok() {
            count += 1;
        }
    }

    Ok(count)
}

/// Get items by type (file/folder) with icons.
#[tauri::command]
pub async fn get_items_by_type(
    state: State<'_, AppState>,
    item_type: String,
) -> Result<Vec<AppEntry>, String> {
    let mut result = {
        let conn = state.lock_db()?;
        queries::get_items_by_type(&conn, &item_type).map_err(|e| e.to_string())?
    };
    load_icons(&state, &mut result)?;
    Ok(result)
}

/// Get excluded items by type with icons.
#[tauri::command]
pub async fn get_excluded_items_by_type(
    state: State<'_, AppState>,
    item_type: String,
) -> Result<Vec<AppEntry>, String> {
    let mut result = {
        let conn = state.lock_db()?;
        queries::get_excluded_items_by_type(&conn, &item_type).map_err(|e| e.to_string())?
    };
    load_icons(&state, &mut result)?;
    Ok(result)
}

/// Open file or folder with the OS default handler.
#[tauri::command]
pub async fn open_item(state: State<'_, AppState>, app_id: String) -> Result<(), String> {
    let (exe_path, item_type) = {
        let conn = state.lock_db()?;
        let now = Utc::now().timestamp();
        queries::increment_launch(&conn, &app_id, now).map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT exe_path, item_type FROM apps WHERE id = ?1")
            .map_err(|e| e.to_string())?;
        stmt.query_row(rusqlite::params![app_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)
                    .unwrap_or_else(|_| "app".to_string()),
            ))
        })
        .map_err(|e| e.to_string())?
    };

    #[cfg(target_os = "windows")]
    {
        match item_type.as_str() {
            "folder" => {
                std::process::Command::new("explorer")
                    .arg(&exe_path)
                    .spawn()
                    .map_err(|e| format!("Failed to open folder: {}", e))?;
            }
            _ => {
                std::process::Command::new("cmd")
                    .args(["/C", "start", "", &exe_path])
                    .spawn()
                    .map_err(|e| format!("Failed to open: {}", e))?;
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&exe_path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&exe_path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }

    Ok(())
}

// ── Command items (shell scripts registered in the launcher) ──

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CommandData {
    /// Raw command text (can be multi-line)
    pub cmd: String,
    /// Run with UAC elevation
    #[serde(default)]
    pub admin: bool,
    /// Keep the shell window open after the script finishes
    #[serde(default = "default_true")]
    pub keep_open: bool,
    /// Which shell to use: "cmd" (default) or "pwsh" (PowerShell)
    #[serde(default = "default_shell")]
    pub shell: String,
}

fn default_true() -> bool {
    true
}

fn default_shell() -> String {
    "cmd".to_string()
}

/// Encode a CommandData as a unique string to store in `exe_path`.
/// Format: `cmd://<uuid>|<json>` — the uuid ensures UNIQUE constraint
/// is never violated even if two commands have identical content.
fn encode_command_exe_path(id: &str, data: &CommandData) -> String {
    let json = serde_json::to_string(data).unwrap_or_default();
    format!("cmd://{}|{}", id, json)
}

/// Decode a command exe_path back into CommandData.
fn decode_command_exe_path(exe_path: &str) -> Option<CommandData> {
    let rest = exe_path.strip_prefix("cmd://")?;
    let (_, json) = rest.split_once('|')?;
    serde_json::from_str::<CommandData>(json).ok()
}

/// Register a new command item.
#[tauri::command]
pub async fn register_command(
    state: State<'_, AppState>,
    name: String,
    cmd: String,
    admin: bool,
    keep_open: bool,
    shell: Option<String>,
) -> Result<String, String> {
    let now = Utc::now().timestamp();
    let conn = state.lock_db()?;

    ensure_virtual_folder(&conn, "command-registered", "(Registered Commands)", now)?;

    let id = Uuid::new_v4().to_string();
    let data = CommandData {
        cmd,
        admin,
        keep_open,
        shell: shell.unwrap_or_else(default_shell),
    };
    let exe_path = encode_command_exe_path(&id, &data);

    let entry = AppEntry {
        id: id.clone(),
        name,
        exe_path,
        folder_id: "command-registered".to_string(),
        icon_cache_path: None,
        tags: Vec::new(),
        launch_count: 0,
        last_launched: None,
        is_excluded: false,
        is_renamed: false,
        created_at: now,
        item_type: "command".to_string(),
        icon_base64: None,
    };

    queries::insert_app(&conn, &entry).map_err(|e| e.to_string())?;
    Ok(id)
}

/// Update an existing command item's data (command text + flags).
/// Use `rename_app` to change the display name.
#[tauri::command]
pub async fn update_command(
    state: State<'_, AppState>,
    app_id: String,
    cmd: String,
    admin: bool,
    keep_open: bool,
    shell: Option<String>,
) -> Result<(), String> {
    let conn = state.lock_db()?;
    let data = CommandData {
        cmd,
        admin,
        keep_open,
        shell: shell.unwrap_or_else(default_shell),
    };
    let new_exe_path = encode_command_exe_path(&app_id, &data);
    conn.execute(
        "UPDATE apps SET exe_path = ?1 WHERE id = ?2 AND item_type = 'command'",
        rusqlite::params![new_exe_path, app_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get command data for editing.
#[tauri::command]
pub async fn get_command_data(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<CommandData, String> {
    let conn = state.lock_db()?;
    let exe_path = queries::get_app_exe_path(&conn, &app_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Command not found".to_string())?;
    decode_command_exe_path(&exe_path).ok_or_else(|| "Failed to decode command data".to_string())
}

/// Launch a command item: writes the stored script to a temp .bat file and runs it.
#[tauri::command]
pub async fn launch_command(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<(), String> {
    let data = {
        let conn = state.lock_db()?;
        let now = Utc::now().timestamp();
        queries::increment_launch(&conn, &app_id, now).map_err(|e| e.to_string())?;
        let exe_path = queries::get_app_exe_path(&conn, &app_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Command not found".to_string())?;
        decode_command_exe_path(&exe_path)
            .ok_or_else(|| "Failed to decode command data".to_string())?
    };

    let is_pwsh = data.shell == "pwsh";

    // For .bat we strip blank/comment lines so the cmd window stays clean;
    // for .ps1 we keep the script verbatim (PowerShell handles its own
    // comments and multi-line statements).
    let script_body = if is_pwsh {
        data.cmd.replace('\r', "")
    } else {
        let lines: Vec<&str> = data
            .cmd
            .lines()
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| {
                let t = l.trim_start();
                !t.is_empty() && !t.starts_with("::") && !t.starts_with("REM ")
            })
            .collect();
        if lines.is_empty() {
            return Err("Command is empty".to_string());
        }
        lines.join("\r\n")
    };

    if script_body.trim().is_empty() {
        return Err("Command is empty".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        // Write the commands to a temporary script file. This avoids all the
        // shell-escaping pitfalls of trying to pass multi-statement strings
        // through `cmd /C` or PowerShell `-Command`.
        let script_path = {
            let mut p = std::env::temp_dir();
            let ext = if is_pwsh { "ps1" } else { "bat" };
            p.push(format!("nanogram_cmd_{}.{}", Uuid::new_v4(), ext));
            p
        };

        if is_pwsh {
            // PowerShell handles UTF-8 with BOM correctly; without the BOM,
            // PS5.1 reads the file as CP932 on Japanese systems and mangles
            // multi-byte characters in the script.
            let mut bytes = vec![0xEFu8, 0xBB, 0xBF];
            bytes.extend_from_slice(script_body.as_bytes());
            std::fs::write(&script_path, &bytes)
                .map_err(|e| format!("Failed to write temp .ps1: {}", e))?;
        } else {
            let mut body = String::from("@echo off\r\n");
            body.push_str(&script_body);
            body.push_str("\r\n");
            if data.keep_open {
                body.push_str("echo.\r\n");
                body.push_str("pause\r\n");
            }
            std::fs::write(&script_path, body.as_bytes())
                .map_err(|e| format!("Failed to write temp .bat: {}", e))?;
        }

        let script_path_str = script_path.to_string_lossy().into_owned();

        if is_pwsh {
            // PowerShell.exe with execution policy bypass for one-off scripts.
            // Use -NoExit to keep the prompt open when keep_open is set.
            let pwsh_args: Vec<String> = if data.keep_open {
                vec![
                    "-NoExit".into(),
                    "-NoProfile".into(),
                    "-ExecutionPolicy".into(),
                    "Bypass".into(),
                    "-File".into(),
                    script_path_str.clone(),
                ]
            } else {
                vec![
                    "-NoProfile".into(),
                    "-ExecutionPolicy".into(),
                    "Bypass".into(),
                    "-File".into(),
                    script_path_str.clone(),
                ]
            };

            if data.admin {
                run_elevated("powershell.exe", &pwsh_args)?;
            } else {
                // Detach via `cmd /C start` so the PowerShell window is
                // independent of the launcher process.
                let mut shell_cmd = std::process::Command::new("cmd");
                shell_cmd.arg("/C").arg("start").arg("").arg("powershell.exe");
                for a in &pwsh_args {
                    shell_cmd.arg(a);
                }
                shell_cmd
                    .spawn()
                    .map_err(|e| format!("Failed to launch PowerShell: {}", e))?;
            }
        } else if data.admin {
            // .bat with UAC: ShellExecuteW handles cmd.exe invocation for us.
            run_elevated_bat(&script_path_str)?;
        } else {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", "cmd", "/C", &script_path_str])
                .spawn()
                .map_err(|e| format!("Failed to launch command: {}", e))?;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (script_body, data, is_pwsh);
        return Err("Command items are only supported on Windows".to_string());
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn run_elevated_bat(bat_path: &str) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let file: Vec<u16> = bat_path.encode_utf16().chain(std::iter::once(0)).collect();

    let rc = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns HINSTANCE; values <= 32 indicate an error.
    if rc.0 as isize <= 32 {
        return Err(format!("ShellExecuteW failed (code {})", rc.0 as isize));
    }
    Ok(())
}

/// Run a program with arguments under UAC elevation (e.g. PowerShell.exe + script args).
#[cfg(target_os = "windows")]
fn run_elevated(program: &str, args: &[String]) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    // Quote any arg containing whitespace so PowerShell parses it as one token.
    let params: String = args
        .iter()
        .map(|a| {
            if a.chars().any(char::is_whitespace) && !a.starts_with('"') {
                format!("\"{}\"", a)
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let file: Vec<u16> = program.encode_utf16().chain(std::iter::once(0)).collect();
    let parameters: Vec<u16> = params.encode_utf16().chain(std::iter::once(0)).collect();

    let rc = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };

    if rc.0 as isize <= 32 {
        return Err(format!("ShellExecuteW failed (code {})", rc.0 as isize));
    }
    Ok(())
}

/// Launch an app with administrator privileges (UAC elevation).
/// Works for .exe and .lnk — resolves shortcut targets first.
#[tauri::command]
pub async fn launch_app_as_admin(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<(), String> {
    let exe_path = {
        let conn = state.lock_db()?;
        let now = Utc::now().timestamp();
        queries::increment_launch(&conn, &app_id, now).map_err(|e| e.to_string())?;
        queries::get_app_exe_path(&conn, &app_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "App not found".to_string())?
    };

    #[cfg(target_os = "windows")]
    {
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        // Resolve .lnk shortcut to its actual target exe
        let target = {
            let p = std::path::Path::new(&exe_path);
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext == "lnk" {
                crate::scanner::resolve_single_lnk(p)
                    .map(|a| a.exe_path)
                    .unwrap_or(exe_path)
            } else {
                exe_path
            }
        };

        let verb: Vec<u16> = "runas\0".encode_utf16().collect();
        let file: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();

        let rc = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(file.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };

        if rc.0 as isize <= 32 {
            return Err(format!("ShellExecuteW failed (code {})", rc.0 as isize));
        }
    }

    #[cfg(not(target_os = "windows"))]
    return Err("Admin launch is only supported on Windows".to_string());

    Ok(())
}

/// Reveal an app's executable in Explorer (opens the containing directory
/// with the file selected). Resolves .lnk shortcuts to their target exe first.
#[tauri::command]
pub async fn reveal_in_explorer(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<(), String> {
    let exe_path = {
        let conn = state.lock_db()?;
        queries::get_app_exe_path(&conn, &app_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "App not found".to_string())?
    };

    // Resolve .lnk shortcut to its actual target exe so we reveal the real file
    let target = {
        let p = std::path::Path::new(&exe_path);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if ext == "lnk" {
            crate::scanner::resolve_single_lnk(p)
                .map(|a| a.exe_path)
                .unwrap_or(exe_path)
        } else {
            exe_path
        }
    };

    if !std::path::Path::new(&target).exists() {
        return Err(format!("File not found: {}", target));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", target))
            .spawn()
            .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    return Err("Reveal in Explorer is only supported on Windows".to_string());

    Ok(())
}

/// Open folder with a specific program.
#[tauri::command]
pub async fn open_folder_with(
    _state: State<'_, AppState>,
    folder_path: String,
    program: String,
) -> Result<(), String> {
    std::process::Command::new(&program)
        .arg(&folder_path)
        .spawn()
        .map_err(|e| format!("Failed to open with {}: {}", program, e))?;
    Ok(())
}

fn load_icons(state: &AppState, items: &mut [AppEntry]) -> Result<(), String> {
    let mut cache = state.icon_mem_cache.lock().map_err(|e| e.to_string())?;
    for item in items.iter_mut() {
        if let Some(ref path) = item.icon_cache_path {
            if let Some(cached) = cache.get(path) {
                item.icon_base64 = Some(cached.clone());
            } else if let Some(b64) = icons::load_icon_as_base64(path) {
                cache.insert(path.clone(), b64.clone());
                item.icon_base64 = Some(b64);
            }
        }
    }
    Ok(())
}
