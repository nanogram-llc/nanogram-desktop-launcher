import { useState, useEffect, useCallback } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { AppEntry } from "../../lib/types";
import * as api from "../../lib/tauri";
import AppRow from "./AppRow";

type SubTab = "active" | "excluded";

function AppTable() {
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [excludedApps, setExcludedApps] = useState<AppEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [subTab, setSubTab] = useState<SubTab>("active");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [toast, setToast] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3000);
  };

  const loadApps = useCallback(async () => {
    try {
      const allApps = await api.getApps(null, 1000);
      setApps(allApps);
    } catch (e) {
      console.error("Failed to load apps:", e);
    }
  }, []);

  const loadExcludedApps = useCallback(async () => {
    try {
      const excluded = await api.getExcludedApps();
      setExcludedApps(excluded);
    } catch (e) {
      console.error("Failed to load excluded apps:", e);
    }
  }, []);

  useEffect(() => {
    loadApps();
    loadExcludedApps();
  }, [loadApps, loadExcludedApps]);

  // Drag & drop listener
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(async (event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") {
        setDragOver(true);
      } else if (event.payload.type === "leave") {
        setDragOver(false);
      } else if (event.payload.type === "drop") {
        setDragOver(false);
        const paths = event.payload.paths;
        if (paths.length === 0) return;

        const validPaths = paths.filter((p) => {
          const lower = p.toLowerCase();
          return lower.endsWith(".exe") || lower.endsWith(".lnk");
        });

        if (validPaths.length === 0) {
          showToast("exe または lnk ファイルをドロップしてください");
          return;
        }

        try {
          const count = await api.registerFiles(validPaths);
          if (count > 0) {
            showToast(`${count} 件のアプリを登録しました`);
            await loadApps();
          } else {
            showToast("新規のアプリはありませんでした");
          }
        } catch (e) {
          console.error("Failed to register dropped files:", e);
          showToast("登録に失敗しました");
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loadApps]);

  // Clear selection when switching tabs
  useEffect(() => {
    setSelected(new Set());
  }, [subTab]);

  const currentApps = subTab === "active" ? apps : excludedApps;

  const filteredApps = currentApps.filter((app) => {
    if (!filter.trim()) return true;
    const q = filter.toLowerCase();
    return (
      app.name.toLowerCase().includes(q) ||
      app.tags.some((t) => t.toLowerCase().includes(q)) ||
      app.exe_path.toLowerCase().includes(q)
    );
  });

  const toggleSelect = (appId: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(appId)) next.delete(appId);
      else next.add(appId);
      return next;
    });
  };

  const toggleSelectAll = () => {
    if (selected.size === filteredApps.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(filteredApps.map((a) => a.id)));
    }
  };

  const handleRename = async (appId: string, newName: string) => {
    try {
      await api.renameApp(appId, newName);
      const updater = (prev: AppEntry[]) =>
        prev.map((a) => (a.id === appId ? { ...a, name: newName, is_renamed: true } : a));
      setApps(updater);
      setExcludedApps(updater);
    } catch (e) {
      console.error("Failed to rename app:", e);
    }
  };

  const handleSetTags = async (appId: string, tags: string[]) => {
    try {
      await api.setTags(appId, tags);
      const updater = (prev: AppEntry[]) =>
        prev.map((a) => (a.id === appId ? { ...a, tags } : a));
      setApps(updater);
      setExcludedApps(updater);
    } catch (e) {
      console.error("Failed to set tags:", e);
    }
  };

  const handleExclude = async (appId: string) => {
    try {
      await api.excludeApp(appId);
      const app = apps.find((a) => a.id === appId);
      if (app) {
        setApps((prev) => prev.filter((a) => a.id !== appId));
        setExcludedApps((prev) => [...prev, { ...app, is_excluded: true }]);
      }
    } catch (e) {
      console.error("Failed to exclude app:", e);
    }
  };

  const handleInclude = async (appId: string) => {
    try {
      await api.includeApp(appId);
      const app = excludedApps.find((a) => a.id === appId);
      if (app) {
        setExcludedApps((prev) => prev.filter((a) => a.id !== appId));
        setApps((prev) => [...prev, { ...app, is_excluded: false }]);
      }
    } catch (e) {
      console.error("Failed to include app:", e);
    }
  };

  const handleLaunch = async (appId: string) => {
    try {
      await api.launchApp(appId);
    } catch (e) {
      console.error("Failed to launch app:", e);
    }
  };

  const handleBulkExclude = async () => {
    if (selected.size === 0) return;
    const ids = Array.from(selected);
    try {
      await api.bulkExcludeApps(ids);
      const moved = apps.filter((a) => selected.has(a.id));
      setApps((prev) => prev.filter((a) => !selected.has(a.id)));
      setExcludedApps((prev) => [
        ...prev,
        ...moved.map((a) => ({ ...a, is_excluded: true })),
      ]);
      showToast(`${ids.length} 件のアプリを除外しました`);
      setSelected(new Set());
    } catch (e) {
      console.error("Failed to bulk exclude:", e);
    }
  };

  const handleBulkInclude = async () => {
    if (selected.size === 0) return;
    const ids = Array.from(selected);
    try {
      await api.bulkIncludeApps(ids);
      const moved = excludedApps.filter((a) => selected.has(a.id));
      setExcludedApps((prev) => prev.filter((a) => !selected.has(a.id)));
      setApps((prev) => [
        ...prev,
        ...moved.map((a) => ({ ...a, is_excluded: false })),
      ]);
      showToast(`${ids.length} 件のアプリを復旧しました`);
      setSelected(new Set());
    } catch (e) {
      console.error("Failed to bulk include:", e);
    }
  };

  return (
    <div className="space-y-4 relative">
      {/* Toast */}
      {toast && (
        <div className="fixed top-3 right-3 z-50 px-4 py-2 bg-gray-800 dark:bg-gray-200 text-white dark:text-gray-900 text-sm rounded-lg shadow-lg animate-fade-in">
          {toast}
        </div>
      )}

      {/* Drag overlay */}
      {dragOver && (
        <div className="fixed inset-0 z-40 bg-blue-500/10 backdrop-blur-sm flex items-center justify-center pointer-events-none">
          <div className="px-8 py-6 bg-white dark:bg-gray-800 rounded-2xl shadow-2xl border-2 border-dashed border-blue-400 dark:border-blue-500">
            <div className="text-center">
              <svg className="w-12 h-12 mx-auto mb-3 text-blue-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 4v16m8-8H4" />
              </svg>
              <div className="text-sm font-medium text-gray-700 dark:text-gray-300">
                exe / lnk ファイルをドロップして登録
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Sub tabs */}
      <div className="flex items-center gap-1 bg-gray-100 dark:bg-gray-800 rounded-lg p-0.5">
        <button
          onClick={() => setSubTab("active")}
          className={`flex-1 px-3 py-1.5 text-sm rounded-md transition-colors ${
            subTab === "active"
              ? "bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm font-medium"
              : "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300"
          }`}
        >
          アプリ一覧
          <span className="ml-1 text-xs text-gray-400">({apps.length})</span>
        </button>
        <button
          onClick={() => setSubTab("excluded")}
          className={`flex-1 px-3 py-1.5 text-sm rounded-md transition-colors ${
            subTab === "excluded"
              ? "bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm font-medium"
              : "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300"
          }`}
        >
          除外アプリ
          <span className="ml-1 text-xs text-gray-400">({excludedApps.length})</span>
        </button>
      </div>

      {/* Search + bulk actions */}
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="アプリを検索..."
          className="flex-1 px-3 py-2 text-sm bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 outline-none focus:border-blue-400"
        />
        {selected.size > 0 && (
          subTab === "active" ? (
            <button
              onClick={handleBulkExclude}
              className="px-3 py-2 text-xs font-medium bg-yellow-500 text-white rounded-lg hover:bg-yellow-600 transition-colors whitespace-nowrap"
            >
              {selected.size}件を除外
            </button>
          ) : (
            <button
              onClick={handleBulkInclude}
              className="px-3 py-2 text-xs font-medium bg-green-500 text-white rounded-lg hover:bg-green-600 transition-colors whitespace-nowrap"
            >
              {selected.size}件を復旧
            </button>
          )
        )}
      </div>

      {/* Select all */}
      {filteredApps.length > 0 && (
        <label className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400 cursor-pointer select-none">
          <input
            type="checkbox"
            checked={selected.size === filteredApps.length && filteredApps.length > 0}
            onChange={toggleSelectAll}
            className="rounded border-gray-300 dark:border-gray-600 accent-blue-500"
          />
          すべて選択 ({filteredApps.length}件)
        </label>
      )}

      {/* Drop hint when empty */}
      {subTab === "active" && apps.length === 0 && !filter && (
        <div className="border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-xl py-8 text-center">
          <svg className="w-10 h-10 mx-auto mb-2 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 4v16m8-8H4" />
          </svg>
          <div className="text-sm text-gray-500 dark:text-gray-400">
            exe / lnk ファイルをここにドロップして登録
          </div>
          <div className="text-xs text-gray-400 dark:text-gray-500 mt-1">
            またはフォルダタブからフォルダを追加
          </div>
        </div>
      )}

      {/* App list */}
      <div className="space-y-1 max-h-[360px] overflow-y-auto">
        {filteredApps.length === 0 && (subTab !== "active" || apps.length > 0 || filter) ? (
          <div className="text-sm text-gray-500 dark:text-gray-400 py-8 text-center">
            {filter
              ? "該当するアプリが見つかりません"
              : subTab === "excluded"
                ? "除外されたアプリはありません"
                : "アプリが登録されていません"}
          </div>
        ) : (
          filteredApps.map((app) => (
            <AppRow
              key={app.id}
              app={app}
              selected={selected.has(app.id)}
              onToggleSelect={toggleSelect}
              onRename={handleRename}
              onSetTags={handleSetTags}
              onRemove={subTab === "active" ? handleExclude : handleInclude}
              onLaunch={handleLaunch}
              removeMode={subTab === "active" ? "exclude" : "include"}
            />
          ))
        )}
      </div>
    </div>
  );
}

export default AppTable;
