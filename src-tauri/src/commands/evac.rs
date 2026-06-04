//! DB evacuation mode.
//!
//! The launcher DB lives next to the EXE in a cloud-synced folder (Dropbox).
//! SQLite's frequent whole-file rewrites fight the cloud client for the file
//! lock and trigger constant re-uploads, which stalls sync. Evacuation moves a
//! working copy to %LOCALAPPDATA% and points the app at it, leaving the Dropbox
//! copy untouched so it can finish syncing. "Merge" reconciles the local
//! changes back into the Dropbox copy; "discard" throws them away.
//!
//! Evacuation state is machine-local (a marker file), never stored in the
//! shared settings.json, so multiple PCs sharing the Dropbox folder stay
//! independent.

use crate::AppState;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize, Clone)]
pub struct EvacStatus {
    pub evacuated: bool,
}

#[derive(Serialize, Clone)]
pub struct MergeResult {
    /// "overwrite" (Dropbox side untouched) or "merge" (3-way reconcile).
    pub strategy: String,
    pub apps_added: u32,
    pub apps_updated: u32,
    pub folders_added: u32,
}

pub fn evac_status(state: &AppState) -> EvacStatus {
    EvacStatus {
        evacuated: state.is_evacuated(),
    }
}

// ── Core operations (also called directly from the tray) ──

/// Copy the Dropbox DB to the local working copy + snapshot, then flip the
/// marker so the app reads the local copy. No-op if already evacuated.
pub fn do_evacuate(state: &AppState) -> Result<(), String> {
    if state.is_evacuated() {
        return Ok(());
    }
    // Release any handle on the Dropbox DB before copying it.
    state.close_db();

    if !state.dropbox_db_path.exists() {
        return Err("Dropbox側のDBが見つかりません".into());
    }
    if let Some(dir) = state.local_db_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::copy(&state.dropbox_db_path, &state.local_db_path).map_err(|e| e.to_string())?;
    std::fs::copy(&state.dropbox_db_path, &state.evac_snapshot_path).map_err(|e| e.to_string())?;
    // Marker last — its existence is what flips active_db_path() to local.
    std::fs::write(&state.evac_marker_path, b"1").map_err(|e| e.to_string())?;
    Ok(())
}

/// Reconcile the local working copy back into the Dropbox copy and leave
/// evacuation mode. Auto-detects whether a 3-way merge is needed.
pub fn do_merge(state: &AppState) -> Result<MergeResult, String> {
    if !state.is_evacuated() {
        return Err("退避モードではありません".into());
    }
    state.close_db();

    let dropbox = &state.dropbox_db_path;
    let local = &state.local_db_path;
    let base = &state.evac_snapshot_path;

    if !local.exists() {
        return Err("ローカルの退避DBが見つかりません".into());
    }

    // Overwrite is correct (and lossless) when the Dropbox copy hasn't changed
    // since evacuation, or when we have no base to merge against.
    let overwrite = !dropbox.exists() || !base.exists() || files_equal(dropbox, base)?;

    let result = if overwrite {
        std::fs::copy(local, dropbox).map_err(|e| e.to_string())?;
        MergeResult {
            strategy: "overwrite".into(),
            apps_added: 0,
            apps_updated: 0,
            folders_added: 0,
        }
    } else {
        merge_3way(base, local, dropbox)?
    };

    // Leave evacuation mode: drop the marker first, then clean up working files.
    let _ = std::fs::remove_file(&state.evac_marker_path);
    let _ = std::fs::remove_file(&state.local_db_path);
    let _ = std::fs::remove_file(&state.evac_snapshot_path);
    Ok(result)
}

/// Throw away the local working copy and leave evacuation mode. The Dropbox
/// copy is left exactly as it was at evacuation time.
pub fn do_discard(state: &AppState) -> Result<(), String> {
    if !state.is_evacuated() {
        return Err("退避モードではありません".into());
    }
    state.close_db();
    let _ = std::fs::remove_file(&state.evac_marker_path);
    let _ = std::fs::remove_file(&state.local_db_path);
    let _ = std::fs::remove_file(&state.evac_snapshot_path);
    Ok(())
}

fn files_equal(a: &Path, b: &Path) -> Result<bool, String> {
    let da = std::fs::read(a).map_err(|e| e.to_string())?;
    let db = std::fs::read(b).map_err(|e| e.to_string())?;
    Ok(da == db)
}

// ── 3-way merge ──

#[derive(Clone)]
struct AppRow {
    id: String,
    name: String,
    exe_path: String,
    folder_id: String,
    icon_cache_path: Option<String>,
    tags: String,
    launch_count: i64,
    last_launched: Option<i64>,
    is_excluded: i64,
    is_renamed: i64,
    created_at: i64,
    item_type: Option<String>,
}

struct FolderRow {
    id: String,
    path: String,
    added_at: i64,
    last_scanned: Option<i64>,
}

fn read_apps(conn: &Connection) -> Result<Vec<AppRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, exe_path, folder_id, icon_cache_path, tags, launch_count, \
             last_launched, is_excluded, is_renamed, created_at, item_type FROM apps",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(AppRow {
                id: r.get(0)?,
                name: r.get(1)?,
                exe_path: r.get(2)?,
                folder_id: r.get(3)?,
                icon_cache_path: r.get(4)?,
                tags: r.get::<_, Option<String>>(5)?.unwrap_or_else(|| "[]".to_string()),
                launch_count: r.get(6)?,
                last_launched: r.get(7)?,
                is_excluded: r.get(8)?,
                is_renamed: r.get(9)?,
                created_at: r.get(10)?,
                item_type: r.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn read_folders(conn: &Connection) -> Result<Vec<FolderRow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, path, added_at, last_scanned FROM folders")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(FolderRow {
                id: r.get(0)?,
                path: r.get(1)?,
                added_at: r.get(2)?,
                last_scanned: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn read_settings(conn: &Connection) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT key, value FROM settings")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn max_opt(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (Some(x), None) => Some(x),
        (None, b) => b,
    }
}

/// Additive 3-way merge of `ours` (local) into `theirs` (Dropbox), using `base`
/// (the evacuation-time snapshot) as the common ancestor. Deletions are NOT
/// propagated — the merge only adds rows and reconciles fields, which is the
/// safe choice for a launcher.
fn merge_3way(base_path: &Path, ours_path: &Path, theirs_path: &Path) -> Result<MergeResult, String> {
    let base_conn = Connection::open(base_path).map_err(|e| e.to_string())?;
    let ours_conn = Connection::open(ours_path).map_err(|e| e.to_string())?;
    let mut theirs_conn = Connection::open(theirs_path).map_err(|e| e.to_string())?;

    let base_apps: HashMap<String, AppRow> = read_apps(&base_conn)?
        .into_iter()
        .map(|a| (a.id.clone(), a))
        .collect();
    let ours_apps = read_apps(&ours_conn)?;
    let ours_folders = read_folders(&ours_conn)?;
    let ours_settings = read_settings(&ours_conn)?;

    let theirs_apps = read_apps(&theirs_conn)?;
    let theirs_by_id: HashMap<String, AppRow> =
        theirs_apps.iter().map(|a| (a.id.clone(), a.clone())).collect();
    let theirs_exe_to_id: HashMap<String, String> = theirs_apps
        .iter()
        .map(|a| (a.exe_path.clone(), a.id.clone()))
        .collect();

    let theirs_folders = read_folders(&theirs_conn)?;
    let theirs_folder_ids: HashSet<String> =
        theirs_folders.iter().map(|f| f.id.clone()).collect();
    let theirs_path_to_id: HashMap<String, String> = theirs_folders
        .iter()
        .map(|f| (f.path.clone(), f.id.clone()))
        .collect();

    let theirs_setting_keys: HashSet<String> =
        read_settings(&theirs_conn)?.into_iter().map(|(k, _)| k).collect();

    let mut apps_added = 0u32;
    let mut apps_updated = 0u32;
    let mut folders_added = 0u32;

    let tx = theirs_conn.transaction().map_err(|e| e.to_string())?;

    // ── Folders: union. Same path under a different id is remapped so apps
    //    referencing the local folder id still satisfy the FK on theirs. ──
    let mut folder_id_remap: HashMap<String, String> = HashMap::new();
    for f in &ours_folders {
        if theirs_folder_ids.contains(&f.id) {
            continue; // already present
        }
        if let Some(tid) = theirs_path_to_id.get(&f.path) {
            folder_id_remap.insert(f.id.clone(), tid.clone());
        } else {
            tx.execute(
                "INSERT INTO folders (id, path, added_at, last_scanned) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![f.id, f.path, f.added_at, f.last_scanned],
            )
            .map_err(|e| e.to_string())?;
            folders_added += 1;
        }
    }

    // ── Apps ──
    for o in &ours_apps {
        let base_row = base_apps.get(&o.id);
        let base_count = base_row.map(|b| b.launch_count).unwrap_or(0);
        let delta = (o.launch_count - base_count).max(0);

        if let Some(t) = theirs_by_id.get(&o.id) {
            // Same id in theirs — reconcile in place.
            let new_count = t.launch_count + delta;
            let new_last = max_opt(o.last_launched, t.last_launched);
            // Field-level: ours wins if it changed from base, else keep theirs.
            let name = if base_row.map_or(true, |b| b.name != o.name) {
                o.name.clone()
            } else {
                t.name.clone()
            };
            let tags = if base_row.map_or(true, |b| b.tags != o.tags) {
                o.tags.clone()
            } else {
                t.tags.clone()
            };
            let icon = if base_row.map_or(true, |b| b.icon_cache_path != o.icon_cache_path) {
                o.icon_cache_path.clone()
            } else {
                t.icon_cache_path.clone()
            };
            let excl = if base_row.map_or(true, |b| b.is_excluded != o.is_excluded) {
                o.is_excluded
            } else {
                t.is_excluded
            };
            let renamed = if base_row.map_or(true, |b| b.is_renamed != o.is_renamed) {
                o.is_renamed
            } else {
                t.is_renamed
            };
            tx.execute(
                "UPDATE apps SET name=?1, tags=?2, icon_cache_path=?3, launch_count=?4, \
                 last_launched=?5, is_excluded=?6, is_renamed=?7 WHERE id=?8",
                rusqlite::params![name, tags, icon, new_count, new_last, excl, renamed, o.id],
            )
            .map_err(|e| e.to_string())?;
            apps_updated += 1;
        } else if let Some(tid) = theirs_exe_to_id.get(&o.exe_path) {
            // Different id, same exe_path (UNIQUE) — merge counts into existing.
            let t = &theirs_by_id[tid];
            let new_count = t.launch_count + delta;
            let new_last = max_opt(o.last_launched, t.last_launched);
            tx.execute(
                "UPDATE apps SET launch_count=?1, last_launched=?2 WHERE id=?3",
                rusqlite::params![new_count, new_last, tid],
            )
            .map_err(|e| e.to_string())?;
            apps_updated += 1;
        } else {
            // New app — insert, translating folder_id through the remap.
            let folder_id = folder_id_remap
                .get(&o.folder_id)
                .cloned()
                .unwrap_or_else(|| o.folder_id.clone());
            tx.execute(
                "INSERT INTO apps (id, name, exe_path, folder_id, icon_cache_path, tags, \
                 launch_count, last_launched, is_excluded, is_renamed, created_at, item_type) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                rusqlite::params![
                    o.id,
                    o.name,
                    o.exe_path,
                    folder_id,
                    o.icon_cache_path,
                    o.tags,
                    o.launch_count,
                    o.last_launched,
                    o.is_excluded,
                    o.is_renamed,
                    o.created_at,
                    o.item_type
                ],
            )
            .map_err(|e| e.to_string())?;
            apps_added += 1;
        }
    }

    // ── Settings kv table: add keys missing in theirs. ──
    for (k, v) in &ours_settings {
        if !theirs_setting_keys.contains(k) {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)",
                rusqlite::params![k, v],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(MergeResult {
        strategy: "merge".into(),
        apps_added,
        apps_updated,
        folders_added,
    })
}

// ── Tauri commands (thin wrappers + UI notification) ──

#[tauri::command]
pub async fn get_evac_status(state: State<'_, AppState>) -> Result<EvacStatus, String> {
    Ok(evac_status(state.inner()))
}

#[tauri::command]
pub async fn evacuate(app: AppHandle, state: State<'_, AppState>) -> Result<EvacStatus, String> {
    do_evacuate(state.inner())?;
    let s = evac_status(state.inner());
    let _ = app.emit("evac-changed", s.clone());
    Ok(s)
}

#[tauri::command]
pub async fn merge_evacuation(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MergeResult, String> {
    let r = do_merge(state.inner())?;
    let _ = app.emit("evac-changed", evac_status(state.inner()));
    Ok(r)
}

#[tauri::command]
pub async fn discard_evacuation(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<EvacStatus, String> {
    do_discard(state.inner())?;
    let s = evac_status(state.inner());
    let _ = app.emit("evac-changed", s.clone());
    Ok(s)
}

/// Notify the launcher window that evacuation state changed (used by the tray).
pub fn emit_evac_changed(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = app.emit("evac-changed", evac_status(state.inner()));
}
