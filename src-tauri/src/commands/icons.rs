use crate::db::queries;
use crate::icons;
use crate::AppState;
use std::path::Path;
use tauri::State;

#[tauri::command]
pub async fn get_icon(state: State<'_, AppState>, app_id: String) -> Result<String, String> {
    let icon_path = {
        let conn = state.lock_db()?;
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

/// Snapshot of an icon-extraction target. Holding onto these copies lets us
/// release the DB lock during the (slow) extraction loop.
fn collect_icon_targets(
    state: &AppState,
    include_with_cached: bool,
) -> Result<Vec<(String, String, String)>, String> {
    let conn = state.lock_db()?;
    if include_with_cached {
        queries::get_all_app_icon_targets(&conn).map_err(|e| e.to_string())
    } else {
        queries::get_apps_missing_icon(&conn).map_err(|e| e.to_string())
    }
}

fn extract_for_target(cache_path: &Path, id: &str, exe_path: &str, item_type: &str) -> Option<String> {
    match item_type {
        "file" | "folder" => icons::extract_file_icon_and_cache(exe_path, cache_path, id),
        _ => icons::extract_and_cache_icon(exe_path, cache_path, id),
    }
}

/// Inner implementation of icon regeneration that takes a plain &AppState
/// so it can be called from both Tauri commands and the startup task.
pub fn regenerate_missing_icons_impl(state: &AppState) -> Result<u32, String> {
    let cache_path = Path::new(&state.icon_cache_dir).to_path_buf();
    let targets = collect_icon_targets(state, false)?;

    let mut count = 0u32;
    for (id, exe_path, item_type) in targets {
        // Treat entries whose cached PNG has gone missing on disk as
        // candidates for re-extraction too.
        let already_cached_and_present = {
            let conn = state.lock_db()?;
            queries::get_icon_cache_path(&conn, &id)
                .ok()
                .flatten()
                .map(|p| Path::new(&p).is_file())
                .unwrap_or(false)
        };
        if already_cached_and_present {
            continue;
        }
        if let Some(path) = extract_for_target(&cache_path, &id, &exe_path, &item_type) {
            let conn = state.lock_db()?;
            if queries::update_icon_cache_path(&conn, &id, &path).is_ok() {
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Re-extract icons for any DB apps where extraction previously failed
/// (icon_cache_path IS NULL). Returns the number of icons newly cached.
#[tauri::command]
pub async fn regenerate_missing_icons(state: State<'_, AppState>) -> Result<u32, String> {
    regenerate_missing_icons_impl(state.inner())
}

/// Re-extract icons for *every* app entry, overwriting any existing cached
/// PNG. Use this after improving the extractor to refresh stale or
/// low-quality icons. Returns the number of icons re-cached.
#[tauri::command]
pub async fn regenerate_all_icons(state: State<'_, AppState>) -> Result<u32, String> {
    let cache_path = Path::new(&state.icon_cache_dir).to_path_buf();
    let targets = collect_icon_targets(state.inner(), true)?;

    let mut count = 0u32;
    for (id, exe_path, item_type) in targets {
        if let Some(path) = extract_for_target(&cache_path, &id, &exe_path, &item_type) {
            let conn = state.lock_db()?;
            if queries::update_icon_cache_path(&conn, &id, &path).is_ok() {
                count += 1;
            }
        }
    }
    Ok(count)
}
