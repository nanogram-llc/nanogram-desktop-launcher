use super::models::{AppEntry, Folder, Settings};
use rusqlite::{params, Connection};

// ── Folders ──

pub fn insert_folder(conn: &Connection, folder: &Folder) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO folders (id, path, added_at, last_scanned) VALUES (?1, ?2, ?3, ?4)",
        params![folder.id, folder.path, folder.added_at, folder.last_scanned],
    )?;
    Ok(())
}

pub fn get_folders(conn: &Connection) -> Result<Vec<Folder>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT id, path, added_at, last_scanned FROM folders WHERE id NOT LIKE 'settings-%' AND id NOT LIKE '%-registered' AND id != 'drop-registered' ORDER BY added_at DESC")?;
    let folders = stmt
        .query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                path: row.get(1)?,
                added_at: row.get(2)?,
                last_scanned: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(folders)
}

pub fn delete_folder(conn: &Connection, folder_id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM folders WHERE id = ?1", params![folder_id])?;
    Ok(())
}

pub fn update_folder_scanned(
    conn: &Connection,
    folder_id: &str,
    timestamp: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE folders SET last_scanned = ?1 WHERE id = ?2",
        params![timestamp, folder_id],
    )?;
    Ok(())
}

pub fn get_folder_by_id(
    conn: &Connection,
    folder_id: &str,
) -> Result<Option<Folder>, rusqlite::Error> {
    let mut stmt =
        conn.prepare("SELECT id, path, added_at, last_scanned FROM folders WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![folder_id], |row| {
        Ok(Folder {
            id: row.get(0)?,
            path: row.get(1)?,
            added_at: row.get(2)?,
            last_scanned: row.get(3)?,
        })
    })?;
    Ok(rows.next().transpose()?)
}

// ── Apps ──

pub fn insert_app(conn: &Connection, app: &AppEntry) -> Result<(), rusqlite::Error> {
    let tags_json = serde_json::to_string(&app.tags).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT OR IGNORE INTO apps (id, name, exe_path, folder_id, icon_cache_path, tags, launch_count, last_launched, is_excluded, is_renamed, created_at, item_type)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            app.id,
            app.name,
            app.exe_path,
            app.folder_id,
            app.icon_cache_path,
            tags_json,
            app.launch_count,
            app.last_launched,
            app.is_excluded as i32,
            app.is_renamed as i32,
            app.created_at,
            app.item_type,
        ],
    )?;
    Ok(())
}

pub fn get_all_apps(conn: &Connection) -> Result<Vec<AppEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, exe_path, folder_id, icon_cache_path, tags, launch_count, last_launched, is_excluded, is_renamed, created_at, item_type
         FROM apps WHERE is_excluded = 0 AND (item_type = 'app' OR item_type = 'command' OR item_type IS NULL) ORDER BY launch_count DESC, last_launched DESC"
    )?;
    parse_app_rows(&mut stmt, [])
}

/// Like get_all_apps but also includes registered files/folders so the
/// launcher can search, rank, and pin them. The settings "apps" tab keeps
/// using get_all_apps to avoid mixing item types.
pub fn get_all_launchable(conn: &Connection) -> Result<Vec<AppEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, exe_path, folder_id, icon_cache_path, tags, launch_count, last_launched, is_excluded, is_renamed, created_at, item_type
         FROM apps WHERE is_excluded = 0 AND (item_type = 'app' OR item_type = 'command' OR item_type = 'file' OR item_type = 'folder' OR item_type IS NULL) ORDER BY launch_count DESC, last_launched DESC"
    )?;
    parse_app_rows(&mut stmt, [])
}

#[allow(dead_code)]
pub fn get_apps_by_folder(
    conn: &Connection,
    folder_id: &str,
) -> Result<Vec<AppEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, exe_path, folder_id, icon_cache_path, tags, launch_count, last_launched, is_excluded, is_renamed, created_at, item_type
         FROM apps WHERE folder_id = ?1 ORDER BY name"
    )?;
    parse_app_rows(&mut stmt, params![folder_id])
}

#[allow(dead_code)]
pub fn get_all_apps_including_excluded(
    conn: &Connection,
) -> Result<Vec<AppEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, exe_path, folder_id, icon_cache_path, tags, launch_count, last_launched, is_excluded, is_renamed, created_at, item_type
         FROM apps ORDER BY name"
    )?;
    parse_app_rows(&mut stmt, [])
}

pub fn get_items_by_type(
    conn: &Connection,
    item_type: &str,
) -> Result<Vec<AppEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, exe_path, folder_id, icon_cache_path, tags, launch_count, last_launched, is_excluded, is_renamed, created_at, item_type
         FROM apps WHERE item_type = ?1 AND is_excluded = 0 ORDER BY name"
    )?;
    parse_app_rows(&mut stmt, params![item_type])
}

pub fn get_excluded_items_by_type(
    conn: &Connection,
    item_type: &str,
) -> Result<Vec<AppEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, exe_path, folder_id, icon_cache_path, tags, launch_count, last_launched, is_excluded, is_renamed, created_at, item_type
         FROM apps WHERE item_type = ?1 AND is_excluded = 1 ORDER BY name"
    )?;
    parse_app_rows(&mut stmt, params![item_type])
}

pub fn get_excluded_apps(conn: &Connection) -> Result<Vec<AppEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, exe_path, folder_id, icon_cache_path, tags, launch_count, last_launched, is_excluded, is_renamed, created_at, item_type
         FROM apps WHERE is_excluded = 1 AND (item_type = 'app' OR item_type IS NULL) ORDER BY name"
    )?;
    parse_app_rows(&mut stmt, [])
}

pub fn bulk_set_excluded(
    conn: &Connection,
    app_ids: &[String],
    excluded: bool,
) -> Result<(), rusqlite::Error> {
    if app_ids.is_empty() {
        return Ok(());
    }
    let placeholders: Vec<String> = app_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let sql = format!(
        "UPDATE apps SET is_excluded = {} WHERE id IN ({})",
        excluded as i32,
        placeholders.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::ToSql> = app_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();
    stmt.execute(params.as_slice())?;
    Ok(())
}

fn parse_app_rows<P: rusqlite::Params>(
    stmt: &mut rusqlite::Statement,
    params: P,
) -> Result<Vec<AppEntry>, rusqlite::Error> {
    let apps = stmt
        .query_map(params, |row| {
            let tags_str: String = row.get(5)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            Ok(AppEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                exe_path: row.get(2)?,
                folder_id: row.get(3)?,
                icon_cache_path: row.get(4)?,
                tags,
                launch_count: row.get(6)?,
                last_launched: row.get(7)?,
                is_excluded: row.get::<_, i32>(8)? != 0,
                is_renamed: row.get::<_, i32>(9)? != 0,
                created_at: row.get(10)?,
                item_type: row
                    .get::<_, String>(11)
                    .unwrap_or_else(|_| "app".to_string()),
                icon_base64: None,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(apps)
}

pub fn delete_app(conn: &Connection, app_id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM apps WHERE id = ?1", params![app_id])?;
    Ok(())
}

pub fn rename_app(conn: &Connection, app_id: &str, new_name: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE apps SET name = ?1, is_renamed = 1 WHERE id = ?2",
        params![new_name, app_id],
    )?;
    Ok(())
}

pub fn set_tags(conn: &Connection, app_id: &str, tags: &[String]) -> Result<(), rusqlite::Error> {
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "UPDATE apps SET tags = ?1 WHERE id = ?2",
        params![tags_json, app_id],
    )?;
    Ok(())
}

pub fn set_excluded(
    conn: &Connection,
    app_id: &str,
    excluded: bool,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE apps SET is_excluded = ?1 WHERE id = ?2",
        params![excluded as i32, app_id],
    )?;
    Ok(())
}

pub fn increment_launch(
    conn: &Connection,
    app_id: &str,
    timestamp: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE apps SET launch_count = launch_count + 1, last_launched = ?1 WHERE id = ?2",
        params![timestamp, app_id],
    )?;
    Ok(())
}

pub fn get_app_exe_path(
    conn: &Connection,
    app_id: &str,
) -> Result<Option<String>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT exe_path FROM apps WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![app_id], |row| row.get::<_, String>(0))?;
    Ok(rows.next().transpose()?)
}

pub fn update_icon_cache_path(
    conn: &Connection,
    app_id: &str,
    icon_cache_path: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE apps SET icon_cache_path = ?1 WHERE id = ?2",
        params![icon_cache_path, app_id],
    )?;
    Ok(())
}

/// Apps that need icon (re)extraction: icon_cache_path is NULL and item is
/// not a registered command (commands use a frontend-rendered icon).
pub fn get_apps_missing_icon(
    conn: &Connection,
) -> Result<Vec<(String, String, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, exe_path, COALESCE(item_type, 'app') FROM apps
         WHERE icon_cache_path IS NULL
           AND (item_type IS NULL OR item_type IN ('app', 'file', 'folder'))",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Every non-command app entry, suitable for full icon re-extraction.
pub fn get_all_app_icon_targets(
    conn: &Connection,
) -> Result<Vec<(String, String, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, exe_path, COALESCE(item_type, 'app') FROM apps
         WHERE item_type IS NULL OR item_type IN ('app', 'file', 'folder')",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_icon_cache_path(
    conn: &Connection,
    app_id: &str,
) -> Result<Option<String>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT icon_cache_path FROM apps WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![app_id], |row| row.get::<_, Option<String>>(0))?;
    Ok(rows.next().transpose()?.flatten())
}

pub fn is_exe_path_registered(conn: &Connection, exe_path: &str) -> Result<bool, rusqlite::Error> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM apps WHERE exe_path = ?1",
        params![exe_path],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn is_exe_path_excluded(conn: &Connection, exe_path: &str) -> Result<bool, rusqlite::Error> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM apps WHERE exe_path = ?1 AND is_excluded = 1",
        params![exe_path],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn delete_non_excluded_apps_by_folder(
    conn: &Connection,
    folder_id: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM apps WHERE folder_id = ?1 AND is_excluded = 0",
        params![folder_id],
    )?;
    Ok(())
}

// ── Settings (legacy, kept for reference) ──

#[allow(dead_code)]
pub fn get_settings(conn: &Connection) -> Result<Settings, rusqlite::Error> {
    let mut settings = Settings::default();

    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (key, value) = row?;
        match key.as_str() {
            "hotkey" => settings.hotkey = value,
            "autostart" => settings.autostart = value == "true",
            "theme" => settings.theme = value,
            "max_results" => settings.max_results = value.parse().unwrap_or(8),
            _ => {}
        }
    }

    Ok(settings)
}

#[allow(dead_code)]
pub fn save_settings(conn: &Connection, settings: &Settings) -> Result<(), rusqlite::Error> {
    let pairs = [
        ("hotkey", settings.hotkey.clone()),
        ("autostart", settings.autostart.to_string()),
        ("theme", settings.theme.clone()),
        ("max_results", settings.max_results.to_string()),
    ];

    for (key, value) in &pairs {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
    }

    Ok(())
}
