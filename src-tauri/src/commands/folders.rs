use crate::commands::scan::{scan_folder_impl, ScanMode};
use crate::db::models::Folder;
use crate::db::queries;
use crate::AppState;
use chrono::Utc;
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

#[derive(Serialize)]
pub struct AddFolderResult {
    pub folder: Folder,
    pub app_count: usize,
}

#[tauri::command]
pub async fn add_folder(
    state: State<'_, AppState>,
    path: String,
) -> Result<AddFolderResult, String> {
    let folder = Folder {
        id: Uuid::new_v4().to_string(),
        path: path.clone(),
        added_at: Utc::now().timestamp(),
        last_scanned: None,
    };

    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        queries::insert_folder(&conn, &folder).map_err(|e| e.to_string())?;
    }

    // Trigger initial scan
    let apps = scan_folder_impl(&state, &folder.id, &path, &ScanMode::AddNew).unwrap_or_default();
    let app_count = apps.len();

    Ok(AddFolderResult { folder, app_count })
}

#[tauri::command]
pub async fn remove_folder(state: State<'_, AppState>, folder_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::delete_folder(&conn, &folder_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_folders(state: State<'_, AppState>) -> Result<Vec<Folder>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    queries::get_folders(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rescan_folder(
    state: State<'_, AppState>,
    folder_id: String,
    mode: ScanMode,
) -> Result<Vec<crate::db::models::AppEntry>, String> {
    let folder_path = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        queries::get_folder_by_id(&conn, &folder_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Folder not found".to_string())?
            .path
    };

    scan_folder_impl(&state, &folder_id, &folder_path, &mode)
}

#[derive(Serialize)]
pub struct StartMenuPaths {
    pub all_users: Option<String>,
    pub current_user: Option<String>,
}

#[tauri::command]
pub async fn get_startmenu_paths() -> Result<StartMenuPaths, String> {
    Ok(StartMenuPaths {
        all_users: get_all_users_startmenu(),
        current_user: get_current_user_startmenu(),
    })
}

fn get_all_users_startmenu() -> Option<String> {
    // C:\ProgramData\Microsoft\Windows\Start Menu\Programs
    let program_data = std::env::var("ProgramData").ok()?;
    let path = std::path::Path::new(&program_data)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");
    if path.is_dir() {
        Some(path.to_string_lossy().to_string())
    } else {
        None
    }
}

fn get_current_user_startmenu() -> Option<String> {
    // %APPDATA%\Microsoft\Windows\Start Menu\Programs
    let appdata = std::env::var("APPDATA").ok()?;
    let path = std::path::Path::new(&appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");
    if path.is_dir() {
        Some(path.to_string_lossy().to_string())
    } else {
        None
    }
}
