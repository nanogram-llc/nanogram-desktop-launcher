use crate::icons;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_icon(state: State<'_, AppState>, app_id: String) -> Result<String, String> {
    let icon_path = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT icon_cache_path FROM apps WHERE id = ?1")
            .map_err(|e| e.to_string())?;
        let path: Option<String> = stmt
            .query_row(rusqlite::params![app_id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        path.ok_or_else(|| "No icon found".to_string())?
    };

    let _ = &state; // keep state alive
    icons::load_icon_as_base64(&icon_path).ok_or_else(|| "Failed to load icon".to_string())
}
