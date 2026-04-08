import { useState, useEffect, useCallback } from "react";
import type { Folder, ScanMode } from "../../lib/types";
import * as api from "../../lib/tauri";

function FolderManager() {
  const [folders, setFolders] = useState<Folder[]>([]);
  const [scanning, setScanning] = useState<string | null>(null);
  const [startMenuPaths, setStartMenuPaths] = useState<api.StartMenuPaths>({
    all_users: null,
    current_user: null,
  });
  const [addingStartMenu, setAddingStartMenu] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3000);
  };

  const loadFolders = useCallback(async () => {
    try {
      const f = await api.getFolders();
      setFolders(f);
    } catch (e) {
      console.error("Failed to load folders:", e);
    }
  }, []);

  useEffect(() => {
    loadFolders();
    api.getStartmenuPaths().then(setStartMenuPaths).catch(() => {});
  }, [loadFolders]);

  const isFolderRegistered = (path: string) =>
    folders.some((f) => f.path.toLowerCase() === path.toLowerCase());

  const handleAddFolder = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    let defaultPath: string | undefined;
    try {
      defaultPath = await api.getExeDir();
    } catch {
      // fallback: no default path
    }
    const selected = await open({
      directory: true,
      multiple: false,
      title: "スキャンするフォルダを選択",
      defaultPath,
    });
    if (!selected) return;

    try {
      const result = await api.addFolder(selected as string);
      await loadFolders();
      showToast(`${result.app_count} 件のアプリを登録しました`);
    } catch (e) {
      console.error("Failed to add folder:", e);
      showToast(`フォルダの追加に失敗しました`);
    }
  };

  const handleAddStartMenu = async (path: string, label: string) => {
    setAddingStartMenu(label);
    try {
      const result = await api.addFolder(path);
      await loadFolders();
      showToast(`「${label}」から ${result.app_count} 件のアプリを登録しました`);
    } catch (e) {
      console.error("Failed to add start menu:", e);
      showToast(`追加に失敗しました`);
    } finally {
      setAddingStartMenu(null);
    }
  };

  const handleRemoveFolder = async (folderId: string) => {
    if (!window.confirm("このフォルダと登録されたアプリを削除しますか？")) return;
    try {
      await api.removeFolder(folderId);
      await loadFolders();
      showToast("フォルダを削除しました");
    } catch (e) {
      console.error("Failed to remove folder:", e);
    }
  };

  const handleRescan = async (folderId: string, mode: ScanMode) => {
    setScanning(folderId);
    try {
      const newApps = await api.rescanFolder(folderId, mode);
      await loadFolders();
      showToast(`${newApps.length} 件のアプリが見つかりました`);
    } catch (e) {
      console.error("Failed to rescan:", e);
      showToast(`再スキャンに失敗しました`);
    } finally {
      setScanning(null);
    }
  };

  const allUsersPath = startMenuPaths.all_users;
  const currentUserPath = startMenuPaths.current_user;
  const showStartMenuSection = allUsersPath || currentUserPath;

  return (
    <div className="space-y-6 relative">
      {/* Toast notification */}
      {toast && (
        <div className="fixed top-3 right-3 z-50 px-4 py-2 bg-gray-800 dark:bg-gray-200 text-white dark:text-gray-900 text-sm rounded-lg shadow-lg animate-fade-in">
          {toast}
        </div>
      )}

      {/* Start Menu quick-add section */}
      {showStartMenuSection && (
        <div className="space-y-3">
          <div>
            <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">スタートメニューから登録</h2>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
              Windowsのスタートメニューに登録されたアプリを一括で取り込みます
            </p>
          </div>
          <div className="flex flex-col gap-2">
            {allUsersPath && (
              <button
                onClick={() => handleAddStartMenu(allUsersPath, "スタートメニュー（全ユーザー）")}
                disabled={addingStartMenu !== null || isFolderRegistered(allUsersPath)}
                className={`flex items-center gap-3 p-3 rounded-xl border text-left transition-colors ${
                  isFolderRegistered(allUsersPath)
                    ? "bg-green-50 dark:bg-green-900/20 border-green-200 dark:border-green-800 opacity-70"
                    : "bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800 hover:bg-blue-100 dark:hover:bg-blue-900/30"
                } disabled:cursor-not-allowed`}
              >
                <div className="w-9 h-9 rounded-lg bg-blue-500 flex items-center justify-center shrink-0">
                  <svg className="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0z" />
                  </svg>
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {isFolderRegistered(allUsersPath) ? "登録済み" : addingStartMenu === "スタートメニュー（全ユーザー）" ? "登録中..." : "スタートメニュー（全ユーザー）のアプリを一括登録"}
                  </div>
                  <div className="text-[10px] text-gray-500 dark:text-gray-400 truncate">{allUsersPath}</div>
                </div>
                {isFolderRegistered(allUsersPath) && (
                  <svg className="w-5 h-5 text-green-500 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                )}
              </button>
            )}
            {currentUserPath && (
              <button
                onClick={() => handleAddStartMenu(currentUserPath, "スタートメニュー（個人）")}
                disabled={addingStartMenu !== null || isFolderRegistered(currentUserPath)}
                className={`flex items-center gap-3 p-3 rounded-xl border text-left transition-colors ${
                  isFolderRegistered(currentUserPath)
                    ? "bg-green-50 dark:bg-green-900/20 border-green-200 dark:border-green-800 opacity-70"
                    : "bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800 hover:bg-blue-100 dark:hover:bg-blue-900/30"
                } disabled:cursor-not-allowed`}
              >
                <div className="w-9 h-9 rounded-lg bg-indigo-500 flex items-center justify-center shrink-0">
                  <svg className="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                  </svg>
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {isFolderRegistered(currentUserPath) ? "登録済み" : addingStartMenu === "スタートメニュー（個人）" ? "登録中..." : "スタートメニュー（個人）のアプリを一括登録"}
                  </div>
                  <div className="text-[10px] text-gray-500 dark:text-gray-400 truncate">{currentUserPath}</div>
                </div>
                {isFolderRegistered(currentUserPath) && (
                  <svg className="w-5 h-5 text-green-500 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                  </svg>
                )}
              </button>
            )}
          </div>
        </div>
      )}

      {/* Manual folder management */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">フォルダ管理</h2>
          <button
            onClick={handleAddFolder}
            className="px-3 py-1.5 text-sm font-medium bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors"
          >
            + フォルダ追加
          </button>
        </div>

        {folders.length === 0 ? (
          <div className="text-sm text-gray-500 dark:text-gray-400 py-4 text-center">
            フォルダが登録されていません
          </div>
        ) : (
          <div className="space-y-2">
            {folders.map((folder) => (
              <div
                key={folder.id}
                className="p-3 rounded-xl bg-gray-100/80 dark:bg-gray-800/80 border border-gray-200/50 dark:border-gray-700/50"
              >
                <div className="flex items-center justify-between mb-2">
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
                      {folder.path}
                    </div>
                    <div className="text-xs text-gray-500 dark:text-gray-400">
                      {folder.last_scanned
                        ? `最終スキャン: ${new Date(folder.last_scanned * 1000).toLocaleString("ja-JP")}`
                        : "未スキャン"}
                    </div>
                  </div>
                  <button
                    onClick={() => handleRemoveFolder(folder.id)}
                    className="p-1.5 text-gray-400 hover:text-red-500 transition-colors shrink-0"
                    title="削除"
                  >
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                  </button>
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={() => handleRescan(folder.id, "add_new")}
                    disabled={scanning === folder.id}
                    className="px-2.5 py-1 text-xs font-medium bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors disabled:opacity-50"
                  >
                    {scanning === folder.id ? "スキャン中..." : "未登録のみ追加"}
                  </button>
                  <button
                    onClick={() => handleRescan(folder.id, "add_all_except_excluded")}
                    disabled={scanning === folder.id}
                    className="px-2.5 py-1 text-xs font-medium bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors disabled:opacity-50"
                  >
                    除外以外を再取得
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default FolderManager;
