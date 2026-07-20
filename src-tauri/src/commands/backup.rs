//! Backup and restore of all user data (settings.json, launcher.db, icons/).
//!
//! Backups are zip archives kept next to the EXE, in two independently managed
//! sets:
//!
//! ```text
//! {exe_dir}/backups/manual/backup-YYYYMMDD-HHMMSS.zip
//! {exe_dir}/backups/auto/backup-YYYYMMDD-HHMMSS.zip   (rotated, 7 kept)
//! ```
//!
//! They are separate directories on purpose: rotation of the automatic set must
//! never delete something the user made deliberately.

use crate::AppState;
use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::State;
use zip::write::SimpleFileOptions;

/// How many automatic backups to keep.
const AUTO_KEEP: usize = 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    /// Unix seconds, parsed from the filename (falls back to file mtime).
    pub created_at: i64,
    /// "manual" or "auto"
    pub kind: String,
}

fn exe_dir(state: &AppState) -> PathBuf {
    Path::new(&state.settings_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn backup_dir(state: &AppState, kind: &str) -> PathBuf {
    exe_dir(state).join("backups").join(kind)
}

/// Parse `backup-YYYYMMDD-HHMMSS.zip` (and the `pre-restore-` variant) back
/// into a timestamp so the UI can sort and display without a second stat call.
fn timestamp_from_name(name: &str) -> Option<i64> {
    let stem = name.strip_suffix(".zip")?;
    let stem = stem
        .strip_prefix("pre-restore-")
        .or_else(|| stem.strip_prefix("backup-"))?;
    let (date, time) = stem.split_once('-')?;
    if date.len() != 8 || time.len() != 6 {
        return None;
    }
    let y: i32 = date[0..4].parse().ok()?;
    let mo: u32 = date[4..6].parse().ok()?;
    let d: u32 = date[6..8].parse().ok()?;
    let h: u32 = time[0..2].parse().ok()?;
    let mi: u32 = time[2..4].parse().ok()?;
    let s: u32 = time[4..6].parse().ok()?;
    Local
        .with_ymd_and_hms(y, mo, d, h, mi, s)
        .single()
        .map(|dt| dt.timestamp())
}

fn add_file(
    zip: &mut zip::ZipWriter<File>,
    src: &Path,
    name: &str,
    opts: SimpleFileOptions,
) -> Result<(), String> {
    if !src.exists() {
        return Ok(());
    }
    let mut buf = Vec::new();
    File::open(src)
        .and_then(|mut f| f.read_to_end(&mut buf))
        .map_err(|e| format!("{} の読み込みに失敗: {}", name, e))?;
    zip.start_file(name, opts).map_err(|e| e.to_string())?;
    zip.write_all(&buf).map_err(|e| e.to_string())?;
    Ok(())
}

/// Write a backup archive. `kind` is "manual" or "auto"; `prefix` lets the
/// pre-restore safety copy be distinguished from a user-made backup.
pub fn write_backup(state: &AppState, kind: &str, prefix: &str) -> Result<BackupInfo, String> {
    // Flush and release the DB so the file on disk is complete and unlocked.
    state.close_db();

    let dir = backup_dir(state, kind);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let now = Local::now();
    let name = format!("{}{}.zip", prefix, now.format("%Y%m%d-%H%M%S"));
    let path = dir.join(&name);

    let file = File::create(&path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let base = exe_dir(state);
    add_file(&mut zip, &base.join("settings.json"), "settings.json", opts)?;
    // Always back up the DB that is currently authoritative.
    add_file(&mut zip, &state.active_db_path(), "launcher.db", opts)?;

    let icons = Path::new(&state.icon_cache_dir);
    if icons.is_dir() {
        let entries = std::fs::read_dir(icons).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(fname) = p.file_name().and_then(|n| n.to_str()) {
                    let entry_name = format!("icons/{}", fname);
                    add_file(&mut zip, &p, &entry_name, opts)?;
                }
            }
        }
    }

    zip.finish().map_err(|e| e.to_string())?;

    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(BackupInfo {
        path: path.to_string_lossy().to_string(),
        name,
        size,
        created_at: now.timestamp(),
        kind: kind.to_string(),
    })
}

/// Keep only the newest `AUTO_KEEP` automatic backups.
fn rotate_auto(state: &AppState) {
    let dir = backup_dir(state, "auto");
    let mut items: Vec<(i64, PathBuf)> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("zip"))
            .map(|p| {
                let ts = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(timestamp_from_name)
                    .unwrap_or(0);
                (ts, p)
            })
            .collect(),
        Err(_) => return,
    };
    if items.len() <= AUTO_KEEP {
        return;
    }
    items.sort_by(|a, b| b.0.cmp(&a.0)); // newest first
    for (_, path) in items.into_iter().skip(AUTO_KEEP) {
        let _ = std::fs::remove_file(path);
    }
}

fn collect(state: &AppState, kind: &str, out: &mut Vec<BackupInfo>) {
    let dir = backup_dir(state, kind);
    let rd = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("zip") {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let meta = entry.metadata().ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let created_at = timestamp_from_name(&name).unwrap_or_else(|| {
            meta.and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });
        out.push(BackupInfo {
            path: path.to_string_lossy().to_string(),
            name,
            size,
            created_at,
            kind: kind.to_string(),
        });
    }
}

/// Create an automatic backup if none exists for today. Called on startup.
pub fn auto_backup_if_due(state: &AppState) -> Result<(), String> {
    let mut items = Vec::new();
    collect(state, "auto", &mut items);

    let today = Local::now().format("%Y%m%d").to_string();
    let has_today = items.iter().any(|b| b.name.contains(&today));
    if has_today {
        return Ok(());
    }

    write_backup(state, "auto", "backup-")?;
    rotate_auto(state);
    Ok(())
}

// ── Tauri commands ──

#[tauri::command]
pub async fn create_backup(state: State<'_, AppState>) -> Result<BackupInfo, String> {
    write_backup(&state, "manual", "backup-")
}

#[tauri::command]
pub async fn list_backups(state: State<'_, AppState>) -> Result<Vec<BackupInfo>, String> {
    let mut items = Vec::new();
    collect(&state, "manual", &mut items);
    collect(&state, "auto", &mut items);
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

#[tauri::command]
pub async fn delete_backup(path: String) -> Result<(), String> {
    std::fs::remove_file(&path).map_err(|e| format!("削除に失敗しました: {}", e))
}

/// Restore settings/DB/icons from a backup archive.
///
/// A safety copy of the current state is written first, so a restore can itself
/// be undone. The caller is expected to prompt for an app restart afterwards —
/// settings, the DB handle and the hotkey config are all reloaded at startup,
/// which is more reliable than re-initialising each of them in place.
#[tauri::command]
pub async fn restore_backup(state: State<'_, AppState>, path: String) -> Result<(), String> {
    // While evacuated the local copy is authoritative and a merge is pending;
    // overwriting either side here would silently discard that work.
    if state.is_evacuated() {
        return Err("DB退避モード中は復元できません。先に「Dropboxへマージ」または「退避を破棄」を実行してください".into());
    }

    let archive_path = PathBuf::from(&path);
    if !archive_path.exists() {
        return Err("バックアップファイルが見つかりません".into());
    }

    // Safety copy before touching anything.
    write_backup(&state, "manual", "pre-restore-")?;

    // close_db() already ran inside write_backup, so the DB file is unlocked.
    let file = File::open(&archive_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| format!("バックアップの読み込みに失敗しました: {}", e))?;

    let base = exe_dir(&state);
    let icons_dir = PathBuf::from(&state.icon_cache_dir);

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let entry_name = entry.name().to_string();

        // Resolve each entry against a known destination rather than joining
        // the archive path directly — a crafted entry must not escape the dir.
        let dest = if entry_name == "settings.json" {
            base.join("settings.json")
        } else if entry_name == "launcher.db" {
            state.dropbox_db_path.clone()
        } else if let Some(icon) = entry_name.strip_prefix("icons/") {
            if icon.is_empty() || icon.contains('/') || icon.contains('\\') {
                continue;
            }
            icons_dir.join(icon)
        } else {
            continue;
        };

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&dest, &buf)
            .map_err(|e| format!("{} の書き込みに失敗: {}", entry_name, e))?;
    }

    // Drop cached icon data URIs so the restored icons are re-read.
    if let Ok(mut cache) = state.icon_mem_cache.lock() {
        cache.clear();
    }

    Ok(())
}
