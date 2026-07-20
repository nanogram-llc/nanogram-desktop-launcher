import { useState, useEffect, useCallback } from "react";
import * as api from "../../lib/tauri";
import type { Settings } from "../../lib/types";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDate(unixSec: number): string {
  if (!unixSec) return "-";
  return new Date(unixSec * 1000).toLocaleString("ja-JP");
}

function BackupManager() {
  const [backups, setBackups] = useState<api.BackupInfo[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [confirmRestore, setConfirmRestore] = useState<api.BackupInfo | null>(null);
  const [restored, setRestored] = useState(false);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 4000);
  };

  const loadBackups = useCallback(async () => {
    try {
      setBackups(await api.listBackups());
    } catch (e) {
      console.error("Failed to list backups:", e);
    }
  }, []);

  useEffect(() => {
    loadBackups();
    api.getSettings().then(setSettings).catch(() => {});
  }, [loadBackups]);

  const handleCreate = async () => {
    setBusy("create");
    try {
      const info = await api.createBackup();
      showToast(`バックアップを作成しました (${formatSize(info.size)})`);
      await loadBackups();
    } catch (e) {
      showToast(`バックアップに失敗しました: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const handleRestore = async (b: api.BackupInfo) => {
    setConfirmRestore(null);
    setBusy(b.path);
    try {
      await api.restoreBackup(b.path);
      setRestored(true);
      await loadBackups();
    } catch (e) {
      showToast(`復元に失敗しました: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const handleDelete = async (b: api.BackupInfo) => {
    setBusy(b.path);
    try {
      await api.deleteBackup(b.path);
      setBackups((prev) => prev.filter((x) => x.path !== b.path));
      showToast("削除しました");
    } catch (e) {
      showToast(`削除に失敗しました: ${e}`);
    } finally {
      setBusy(null);
    }
  };

  const toggleAutoBackup = async () => {
    if (!settings) return;
    const next = { ...settings, auto_backup: !settings.auto_backup };
    setSettings(next);
    try {
      await api.updateSettings(next);
    } catch (e) {
      console.error("Failed to save auto_backup:", e);
    }
  };

  return (
    <div className="space-y-6 relative">
      {toast && (
        <div className="fixed top-3 right-3 z-50 px-4 py-2 bg-gray-800 dark:bg-gray-200 text-white dark:text-gray-900 text-sm rounded-lg shadow-lg animate-fade-in max-w-md">
          {toast}
        </div>
      )}

      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">バックアップ</h2>
        <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
          設定・登録アイテム・アイコンを zip にまとめて保存します（EXE隣の <code>backups/</code> フォルダ）
        </p>
      </div>

      {/* Restart notice after a restore */}
      {restored && (
        <div className="p-3 rounded-xl bg-amber-50 dark:bg-amber-900/20 border border-amber-300 dark:border-amber-700">
          <div className="text-sm font-medium text-amber-800 dark:text-amber-300">
            復元が完了しました
          </div>
          <div className="text-xs text-amber-700 dark:text-amber-400 mt-1">
            変更を反映するには、アプリを一度終了して再起動してください（トレイアイコン →「終了」）。
            復元前の状態は <code>pre-restore-</code> で始まるバックアップから戻せます。
          </div>
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center justify-between p-3 rounded-xl bg-gray-100/80 dark:bg-gray-800/80 border border-gray-200/50 dark:border-gray-700/50">
        <div>
          <div className="text-sm font-medium text-gray-700 dark:text-gray-300">手動バックアップ</div>
          <div className="text-xs text-gray-500 dark:text-gray-400">
            現在の状態をすぐに保存します
          </div>
        </div>
        <button
          onClick={handleCreate}
          disabled={busy === "create"}
          className="px-3 py-1.5 text-sm font-medium accent-bg text-white rounded-lg transition-colors disabled:opacity-50"
        >
          {busy === "create" ? "作成中..." : "今すぐバックアップ"}
        </button>
      </div>

      {/* Auto backup toggle */}
      {settings && (
        <label className="flex items-center justify-between">
          <div>
            <div className="text-sm font-medium text-gray-700 dark:text-gray-300">自動バックアップ</div>
            <div className="text-xs text-gray-500 dark:text-gray-400">
              起動時に1日1回作成し、最新7件を保持します（手動バックアップとは別管理）
            </div>
          </div>
          <button
            onClick={toggleAutoBackup}
            className={`relative w-11 h-6 rounded-full transition-colors shrink-0 ml-4 ${
              settings.auto_backup ? "accent-bg" : "bg-gray-300 dark:bg-gray-600"
            }`}
          >
            <span
              className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform shadow ${
                settings.auto_backup ? "translate-x-5" : ""
              }`}
            />
          </button>
        </label>
      )}

      {/* List */}
      <div>
        <div className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          バックアップ一覧 ({backups.length}件)
        </div>
        {backups.length === 0 ? (
          <div className="border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-xl py-8 text-center text-sm text-gray-500 dark:text-gray-400">
            バックアップがありません
          </div>
        ) : (
          <div className="space-y-1 max-h-[320px] overflow-y-auto">
            {backups.map((b) => (
              <div
                key={b.path}
                className="flex items-center gap-2 p-3 rounded-lg bg-white dark:bg-gray-800/80 border border-gray-200/50 dark:border-gray-700/50"
              >
                <span
                  className={`text-[10px] px-1.5 py-0.5 rounded-full shrink-0 ${
                    b.kind === "auto"
                      ? "bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-400"
                      : "accent-soft-bg accent-text"
                  }`}
                >
                  {b.kind === "auto" ? "自動" : "手動"}
                </span>
                <div className="flex-1 min-w-0">
                  <div className="text-sm text-gray-900 dark:text-gray-100 truncate">
                    {formatDate(b.created_at)}
                  </div>
                  <div className="text-[10px] text-gray-400 dark:text-gray-500 truncate" title={b.path}>
                    {b.name} · {formatSize(b.size)}
                  </div>
                </div>
                <button
                  onClick={() => setConfirmRestore(b)}
                  disabled={busy === b.path}
                  className="px-2.5 py-1 text-xs font-medium bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors disabled:opacity-50 shrink-0"
                >
                  {busy === b.path ? "処理中..." : "復元"}
                </button>
                <button
                  onClick={() => handleDelete(b)}
                  disabled={busy === b.path}
                  className="p-1 text-gray-400 hover:text-red-500 transition-colors disabled:opacity-50 shrink-0"
                  title="削除"
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Restore confirmation */}
      {confirmRestore && (
        <div className="fixed inset-0 z-50 bg-black/30 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-white dark:bg-gray-800 rounded-2xl shadow-2xl p-5 max-w-sm w-full">
            <div className="text-base font-semibold text-gray-900 dark:text-gray-100 mb-2">
              バックアップから復元しますか？
            </div>
            <div className="text-sm text-gray-600 dark:text-gray-400 mb-1">
              {formatDate(confirmRestore.created_at)} の状態に戻します。
            </div>
            <div className="text-xs text-gray-500 dark:text-gray-500 mb-4">
              現在の設定・登録アイテムは上書きされますが、復元直前の状態は自動でバックアップされるため元に戻せます。
            </div>
            <div className="flex gap-2 justify-end">
              <button
                onClick={() => setConfirmRestore(null)}
                className="px-3 py-1.5 text-sm bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
              >
                キャンセル
              </button>
              <button
                onClick={() => handleRestore(confirmRestore)}
                className="px-3 py-1.5 text-sm font-medium accent-bg text-white rounded-lg transition-colors"
              >
                復元する
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default BackupManager;
