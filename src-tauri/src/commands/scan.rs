use crate::db::models::AppEntry;
use crate::db::queries;
use crate::icons;
use crate::scanner;
use crate::AppState;
use chrono::Utc;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Deserialize)]
pub enum ScanMode {
    #[serde(rename = "add_new")]
    AddNew,
    #[serde(rename = "add_all_except_excluded")]
    AddAllExceptExcluded,
}

/// Check if a file name or path matches any of the excluded patterns.
/// Patterns support simple glob: *pattern*, pattern*, *pattern
fn matches_excluded_pattern(name: &str, path: &str, patterns: &[String]) -> bool {
    let name_lower = name.to_lowercase();
    let path_lower = path.to_lowercase().replace('\\', "/");
    for pat in patterns {
        let p = pat.to_lowercase().replace('\\', "/");
        let matched = if p.starts_with('*') && p.ends_with('*') && p.len() > 1 {
            // *keyword* — substring match
            let keyword = &p[1..p.len() - 1];
            name_lower.contains(keyword) || path_lower.contains(keyword)
        } else if p.starts_with('*') {
            // *suffix — ends-with match
            let suffix = &p[1..];
            name_lower.ends_with(suffix) || path_lower.ends_with(suffix)
        } else if p.ends_with('*') {
            // prefix* — starts-with match
            let prefix = &p[..p.len() - 1];
            name_lower.starts_with(prefix) || path_lower.starts_with(prefix)
        } else if p.contains('/') || p.contains(':') {
            // Looks like a path — exact path match
            path_lower == p
        } else {
            // Exact name match
            name_lower == p
        };
        if matched {
            return true;
        }
    }
    false
}

pub fn scan_folder_impl(
    state: &AppState,
    folder_id: &str,
    folder_path: &str,
    mode: &ScanMode,
) -> Result<Vec<AppEntry>, String> {
    let dir = Path::new(folder_path);
    if !dir.is_dir() {
        return Err(format!("Directory not found: {}", folder_path));
    }

    let scanned = scanner::scan_directory(dir);
    let conn = state.lock_db()?;
    let now = Utc::now().timestamp();
    let mut new_apps = Vec::new();

    // Load excluded_patterns from settings
    let excluded_patterns = super::settings::get_excluded_patterns(state);

    match mode {
        ScanMode::AddNew => {
            for app in scanned {
                if queries::is_exe_path_registered(&conn, &app.exe_path).unwrap_or(true) {
                    continue;
                }
                if matches_excluded_pattern(&app.name, &app.exe_path, &excluded_patterns) {
                    continue;
                }
                let entry = create_app_entry(
                    &app.name,
                    &app.exe_path,
                    folder_id,
                    &state.icon_cache_dir,
                    now,
                );
                if queries::insert_app(&conn, &entry).is_ok() {
                    new_apps.push(entry);
                }
            }
        }
        ScanMode::AddAllExceptExcluded => {
            // Delete non-excluded apps for this folder, then re-insert
            let _ = queries::delete_non_excluded_apps_by_folder(&conn, folder_id);
            for app in scanned {
                if queries::is_exe_path_excluded(&conn, &app.exe_path).unwrap_or(false) {
                    continue;
                }
                if matches_excluded_pattern(&app.name, &app.exe_path, &excluded_patterns) {
                    continue;
                }
                let entry = create_app_entry(
                    &app.name,
                    &app.exe_path,
                    folder_id,
                    &state.icon_cache_dir,
                    now,
                );
                if queries::insert_app(&conn, &entry).is_ok() {
                    new_apps.push(entry);
                }
            }
        }
    }

    // Update folder scan timestamp
    let _ = queries::update_folder_scanned(&conn, folder_id, now);

    Ok(new_apps)
}

fn create_app_entry(
    name: &str,
    exe_path: &str,
    folder_id: &str,
    icon_cache_dir: &str,
    now: i64,
) -> AppEntry {
    let id = Uuid::new_v4().to_string();
    let icon_cache_path = icons::extract_and_cache_icon(exe_path, Path::new(icon_cache_dir), &id);

    AppEntry {
        id,
        name: name.to_string(),
        exe_path: exe_path.to_string(),
        folder_id: folder_id.to_string(),
        icon_cache_path,
        tags: Vec::new(),
        launch_count: 0,
        last_launched: None,
        is_excluded: false,
        is_renamed: false,
        created_at: now,
        item_type: "app".to_string(),
        icon_base64: None,
    }
}
