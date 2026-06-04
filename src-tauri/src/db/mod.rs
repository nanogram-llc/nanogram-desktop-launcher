pub mod models;
pub mod queries;

use rusqlite::Connection;
use std::path::Path;

pub fn init_db(db_path: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(db_path)?;

    // DELETE journal mode — Dropbox 同期と相性が悪い .db-wal / .db-shm を
    // 生成しないようにする。ランチャー程度の書き込み頻度では性能差は無視できる。
    // 既存 WAL は journal_mode 切り替え時に自動マージ&削除される。
    conn.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA foreign_keys=ON;")?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS folders (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            added_at INTEGER NOT NULL,
            last_scanned INTEGER
        );

        CREATE TABLE IF NOT EXISTS apps (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            exe_path TEXT NOT NULL UNIQUE,
            folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
            icon_cache_path TEXT,
            tags TEXT DEFAULT '[]',
            launch_count INTEGER DEFAULT 0,
            last_launched INTEGER,
            is_excluded INTEGER DEFAULT 0,
            is_renamed INTEGER DEFAULT 0,
            created_at INTEGER NOT NULL,
            item_type TEXT DEFAULT 'app'
        );

        CREATE INDEX IF NOT EXISTS idx_apps_launch_count ON apps(launch_count DESC);
        CREATE INDEX IF NOT EXISTS idx_apps_last_launched ON apps(last_launched DESC);
        CREATE INDEX IF NOT EXISTS idx_apps_folder_id ON apps(folder_id);
        CREATE INDEX IF NOT EXISTS idx_apps_exe_path ON apps(exe_path);

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )?;

    // Migration: add item_type column if missing (existing DBs)
    let has_item_type: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('apps') WHERE name='item_type'")?
        .query_row([], |row| row.get::<_, i64>(0))
        .map(|c| c > 0)
        .unwrap_or(false);
    if !has_item_type {
        conn.execute_batch("ALTER TABLE apps ADD COLUMN item_type TEXT DEFAULT 'app';")?;
    }

    // Create index after migration ensures column exists
    conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_apps_item_type ON apps(item_type);")?;

    Ok(conn)
}
