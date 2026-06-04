import { useState, useCallback, useEffect, useLayoutEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import SearchBar from "./SearchBar";
import ResultsList from "./ResultsList";
import RecentStrip from "./RecentStrip";
import type { AppEntry, Settings, SectionConfig } from "../../lib/types";
import * as api from "../../lib/tauri";

function LauncherWindow() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<AppEntry[]>([]);
  const [recentApps, setRecentApps] = useState<AppEntry[]>([]);
  const [frequentApps, setFrequentApps] = useState<AppEntry[]>([]);
  const [pinnedApps, setPinnedApps] = useState<AppEntry[]>([]);
  const [pinnedIds, setPinnedIds] = useState<string[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [dragOver, setDragOver] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [maxResults, setMaxResults] = useState(8);
  const [sections, setSections] = useState<SectionConfig[]>([
    { id: "recent", visible: true },
    { id: "frequent", visible: true },
    { id: "pinned", visible: true },
  ]);
  const [pinnedColumns, setPinnedColumns] = useState(9);
  const [pinnedRows, setPinnedRows] = useState(2);
  const [showEvacButton, setShowEvacButton] = useState(false);
  const [evacuated, setEvacuated] = useState(false);
  const searchIdRef = useRef(0);
  const settingsRef = useRef<Settings | null>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  // Last measured CSS pixel height, updated by the ResizeObserver below.
  // applySettings reuses it instead of the formula so that re-opening the
  // launcher doesn't grow it back to a too-generous formula estimate.
  const measuredHRef = useRef<number | null>(null);
  // Layout signature ("cols-rows-sections"). applySettings only triggers a
  // window resize when this changes, so launcher-show with the same
  // settings doesn't kick the window back to formula size.
  const layoutSigRef = useRef<string>("");

  // Search history (persisted in localStorage)
  const HISTORY_KEY = "nanogram-search-history";
  const MAX_HISTORY = 100;
  const [history, setHistory] = useState<string[]>(() => {
    try {
      return JSON.parse(localStorage.getItem(HISTORY_KEY) || "[]");
    } catch { return []; }
  });
  const historyIndexRef = useRef(-1); // -1 = not browsing history
  const savedQueryRef = useRef(""); // query before entering history mode

  const addToHistory = useCallback((q: string) => {
    const trimmed = q.trim();
    if (!trimmed) return;
    setHistory((prev) => {
      const filtered = prev.filter((h) => h !== trimmed);
      const next = [trimmed, ...filtered].slice(0, MAX_HISTORY);
      localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
      return next;
    });
  }, []);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3000);
  };

  const applySettings = useCallback((s: Settings) => {
    settingsRef.current = s;
    setMaxResults(s.max_results);
    setShowEvacButton(!!s.show_evac_button);
    const cols = s.pinned_columns || 9;
    const rows = s.pinned_rows || 2;
    // 古いデータが現在のグリッドサイズより大きい/小さい場合に正規化
    const slots = cols * rows;
    const rawIds = s.pinned_items || [];
    const normalized = Array.from({ length: slots }, (_, i) => rawIds[i] ?? "");
    setPinnedIds(normalized);
    const secs = (s.sections && s.sections.length > 0) ? s.sections : sections;
    setSections(secs);
    setPinnedColumns(cols);
    setPinnedRows(rows);
    // Only resize the window when layout-affecting settings actually
    // changed; otherwise we'd undo the ResizeObserver's measurement on
    // every launcher-show.
    const visibleSections = secs.filter((sec) => sec.visible);
    const sectionCount = visibleSections.length;
    const pinnedVisible = visibleSections.some((sec) => sec.id === "pinned");
    const extraRows = sectionCount - (pinnedVisible ? 1 : 0);
    const totalRows = extraRows + (pinnedVisible ? rows : 0);
    const sig = `${cols}-${rows}-${sectionCount}-${pinnedVisible ? 1 : 0}`;
    if (sig !== layoutSigRef.current) {
      const formulaH = totalRows * 72 + sectionCount * 28 + 128;
      // Prefer the last measured content height; fall back to formula
      // only on the very first call before ResizeObserver has run.
      const h = measuredHRef.current ?? formulaH;
      api
        .setLauncherSize(cols, h, window.devicePixelRatio)
        .catch(console.error);
      layoutSigRef.current = sig;
    }
  }, []);

  const loadRecentFrequent = useCallback(async (cols: number, rows: number, pIds: string[]) => {
    try {
      const apps = await api.getApps(null, 100);
      const recent = apps
        .filter((a) => a.last_launched)
        .sort((a, b) => (b.last_launched ?? 0) - (a.last_launched ?? 0))
        .slice(0, cols);
      const frequent = apps
        .filter((a) => a.launch_count > 0)
        .sort((a, b) => b.launch_count - a.launch_count)
        .slice(0, cols);

      const appMap = new Map(apps.map((a) => [a.id, a]));
      // グリッドサイズに正規化し、DBから消えた古いIDを空文字に掃除
      const slots = cols * rows;
      const cleanedIds = Array.from({ length: slots }, (_, i) => {
        const id = pIds[i] ?? "";
        return id && appMap.has(id) ? id : "";
      });
      const pinned = cleanedIds
        .filter((id) => id !== "")
        .map((id) => appMap.get(id))
        .filter((a): a is AppEntry => !!a);

      setRecentApps(recent);
      setFrequentApps(frequent);
      setPinnedApps(pinned);

      // 掃除で内容が変わった場合はstateと設定ファイルに反映
      const changed =
        cleanedIds.length !== pIds.length ||
        cleanedIds.some((id, i) => id !== (pIds[i] ?? ""));
      if (changed) {
        setPinnedIds(cleanedIds);
        const s = settingsRef.current;
        if (s) {
          const updated = { ...s, pinned_items: cleanedIds };
          settingsRef.current = updated;
          api.updateSettings(updated).catch(console.error);
        }
      }
    } catch {
      // ignore
    }
  }, []);

  // Load settings + recent/frequent on mount, reload when launcher is shown
  useEffect(() => {
    const loadAll = () => {
      api.getSettings().then((s) => {
        applySettings(s);
        loadRecentFrequent(s.pinned_columns || 9, s.pinned_rows || 2, s.pinned_items || []);
      }).catch(() => {
        loadRecentFrequent(pinnedColumns, pinnedRows, pinnedIds);
      });
    };
    loadAll();
    const unlisten = listen("launcher-show", loadAll);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loadRecentFrequent, applySettings]);

  // Listen for real-time settings changes from settings window
  useEffect(() => {
    const unlisten = listen<Settings>("settings-changed", (event) => {
      const s = event.payload;
      applySettings(s);
      loadRecentFrequent(s.pinned_columns || 9, s.pinned_rows || 2, s.pinned_items || []);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [applySettings, loadRecentFrequent]);

  // Evacuation status: fetch on mount + on launcher-show, keep in sync with
  // tray-triggered changes, and surface tray operation results as toasts.
  useEffect(() => {
    const refresh = () => {
      api.getEvacStatus().then((s) => setEvacuated(s.evacuated)).catch(() => {});
    };
    refresh();
    const unlistenShow = listen("launcher-show", refresh);
    const unlistenChanged = listen<{ evacuated: boolean }>("evac-changed", (e) => {
      setEvacuated(e.payload.evacuated);
    });
    const unlistenToast = listen<string>("evac-toast", (e) => {
      showToast(e.payload);
    });
    return () => {
      unlistenShow.then((fn) => fn());
      unlistenChanged.then((fn) => fn());
      unlistenToast.then((fn) => fn());
    };
  }, []);

  // Drag & drop registration on launcher screen
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

        const exeLnk: string[] = [];
        const others: string[] = [];

        for (const p of paths) {
          const lower = p.toLowerCase();
          if (lower.endsWith(".exe") || lower.endsWith(".lnk")) {
            exeLnk.push(p);
          } else {
            others.push(p);
          }
        }

        let totalCount = 0;
        const msgs: string[] = [];

        if (exeLnk.length > 0) {
          try {
            const c = await api.registerFiles(exeLnk);
            totalCount += c;
            if (c > 0) msgs.push(`アプリ ${c}件`);
          } catch { /* ignore */ }
        }
        if (others.length > 0) {
          try {
            const c = await api.registerItems(others, "folder");
            totalCount += c;
            if (c > 0) msgs.push(`フォルダ ${c}件`);
          } catch { /* ignore */ }
          try {
            const c = await api.registerItems(others, "file");
            totalCount += c;
            if (c > 0) msgs.push(`ファイル ${c}件`);
          } catch { /* ignore */ }
        }

        if (totalCount > 0) {
          showToast(`${msgs.join("、")} を登録しました`);
          loadRecentFrequent(pinnedColumns, pinnedRows, pinnedIds);
        } else {
          showToast("新規の登録対象はありませんでした");
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loadRecentFrequent]);

  // Search immediately (no debounce for snappy feel)
  useEffect(() => {
    if (!query.trim()) {
      setResults([]);
      setSelectedIndex(0);
      return;
    }
    const id = ++searchIdRef.current;
    api.getApps(query, maxResults).then((r) => {
      if (id === searchIdRef.current) {
        setResults(r);
        setSelectedIndex(0);
      }
    }).catch(() => {
      if (id === searchIdRef.current) setResults([]);
    });
  }, [query, maxResults]);

  const handleLaunch = useCallback(async (app: AppEntry) => {
    try {
      if (app.item_type === "file" || app.item_type === "folder") {
        await api.openItem(app.id);
      } else if (app.item_type === "command") {
        await api.launchCommand(app.id);
      } else {
        await api.launchApp(app.id);
      }

      // Optimistic update: immediately reflect launch in recent/frequent
      const now = Math.floor(Date.now() / 1000);
      const updated = { ...app, launch_count: app.launch_count + 1, last_launched: now };

      setRecentApps((prev) => {
        const filtered = prev.filter((a) => a.id !== app.id);
        return [updated, ...filtered].slice(0, pinnedColumns);
      });
      setFrequentApps((prev) => {
        const filtered = prev.filter((a) => a.id !== app.id);
        return [updated, ...filtered]
          .sort((a, b) => b.launch_count - a.launch_count)
          .slice(0, pinnedColumns);
      });
      setPinnedApps((prev) =>
        prev.map((a) => (a.id === app.id ? updated : a))
      );

      addToHistory(query);
      historyIndexRef.current = -1;
      setQuery("");
      setResults([]);
      const win = getCurrentWindow();
      await win.hide();
    } catch (e) {
      console.error("Failed to launch app:", e);
    }
  }, [query, addToHistory, pinnedColumns]);

  const savePinnedItems = useCallback(async (newIds: string[]) => {
    try {
      const s = settingsRef.current ?? await api.getSettings();
      const updated = { ...s, pinned_items: newIds };
      settingsRef.current = updated;
      await api.updateSettings(updated);
    } catch (e) {
      console.error("Failed to save pinned items:", e);
    }
  }, []);

  const handleExclude = useCallback(async (app: AppEntry) => {
    try {
      await api.excludeApp(app.id);
      setResults((prev) => prev.filter((a) => a.id !== app.id));
      setRecentApps((prev) => prev.filter((a) => a.id !== app.id));
      setFrequentApps((prev) => prev.filter((a) => a.id !== app.id));
      setPinnedApps((prev) => prev.filter((a) => a.id !== app.id));
      const next = pinnedIds.map((id) => (id === app.id ? "" : id));
      setPinnedIds(next);
      await savePinnedItems(next);
    } catch (e) {
      console.error("Failed to exclude app:", e);
    }
  }, [pinnedIds, savePinnedItems]);

  const handleSetTags = useCallback(async (app: AppEntry, tags: string[]) => {
    try {
      await api.setTags(app.id, tags);
      const updater = (prev: AppEntry[]) =>
        prev.map((a) => (a.id === app.id ? { ...a, tags } : a));
      setResults(updater);
      setRecentApps(updater);
      setFrequentApps(updater);
      setPinnedApps(updater);
    } catch (e) {
      console.error("Failed to set tags:", e);
    }
  }, []);

  const handlePin = useCallback(async (appId: string) => {
    const slots = pinnedColumns * pinnedRows;
    // グリッドサイズに正規化（古いデータの残存を切り捨て、不足分を空文字で埋める）
    const next = Array.from({ length: slots }, (_, i) => pinnedIds[i] ?? "");
    if (next.includes(appId)) return;
    const emptyIdx = next.indexOf("");
    if (emptyIdx === -1) return; // 空きなし=満杯
    next[emptyIdx] = appId;
    setPinnedIds(next);

    const allApps = [...recentApps, ...frequentApps, ...results];
    const app = allApps.find((a) => a.id === appId);
    if (app) {
      setPinnedApps((p) => [...p, app]);
    }
    await savePinnedItems(next);
  }, [pinnedIds, pinnedColumns, pinnedRows, recentApps, frequentApps, results, savePinnedItems]);

  const handleUnpin = useCallback(async (appId: string) => {
    const next = pinnedIds.map((id) => (id === appId ? "" : id));
    setPinnedIds(next);
    setPinnedApps((p) => p.filter((a) => a.id !== appId));
    await savePinnedItems(next);
  }, [pinnedIds, savePinnedItems]);

  const handleReorderPinned = useCallback(async (newIds: string[]) => {
    setPinnedIds(newIds);
    setPinnedApps((prev) => {
      const map = new Map(prev.map((a) => [a.id, a]));
      return newIds.filter((id) => id !== "").map((id) => map.get(id)).filter((a): a is AppEntry => !!a);
    });
    await savePinnedItems(newIds);
  }, [savePinnedItems]);

  const openSettings = useCallback(async () => {
    // If settings window already exists, just focus it
    const existing = await WebviewWindow.getByLabel("settings");
    if (existing) {
      await existing.setFocus();
      return;
    }
    // Disable blur_hide so launcher stays visible
    api.setBlurHide(false);
    // Create new settings window
    const settingsWin = new WebviewWindow("settings", {
      url: "/",
      title: "設定 - nanogram desktop LauncheR",
      width: 680,
      height: 460,
      resizable: false,
      center: true,
      decorations: true,
      alwaysOnTop: true,
    });
    // Re-enable blur_hide when settings window is closed
    settingsWin.once("tauri://destroyed", () => {
      api.setBlurHide(true);
    });
  }, []);

  // ── DB evacuation controls ──
  const handleEvacuate = useCallback(async () => {
    try {
      const s = await api.evacuate();
      setEvacuated(s.evacuated);
      showToast("DBをローカルに退避しました");
    } catch (e) {
      showToast(`退避に失敗: ${e}`);
    }
  }, []);

  const handleMerge = useCallback(async () => {
    try {
      const r = await api.mergeEvacuation();
      setEvacuated(false);
      if (r.strategy === "overwrite") {
        showToast("Dropboxへ反映しました（上書き）");
      } else {
        showToast(`Dropboxへマージ（追加 ${r.apps_added} / 更新 ${r.apps_updated} / フォルダ ${r.folders_added}）`);
      }
    } catch (e) {
      showToast(`マージに失敗: ${e}`);
    }
  }, []);

  const handleDiscard = useCallback(async () => {
    try {
      const s = await api.discardEvacuation();
      setEvacuated(s.evacuated);
      showToast("退避を破棄しました");
    } catch (e) {
      showToast(`破棄に失敗: ${e}`);
    }
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const items = query.trim() ? results : [];

      // Ctrl+Up/Down: history navigation
      if (e.ctrlKey && e.key === "ArrowUp" && history.length > 0) {
        e.preventDefault();
        if (historyIndexRef.current === -1) {
          savedQueryRef.current = query;
        }
        const nextIdx = Math.min(historyIndexRef.current + 1, history.length - 1);
        historyIndexRef.current = nextIdx;
        setQuery(history[nextIdx]);
        return;
      }
      if (e.ctrlKey && e.key === "ArrowDown" && historyIndexRef.current >= 0) {
        e.preventDefault();
        const nextIdx = historyIndexRef.current - 1;
        historyIndexRef.current = nextIdx;
        setQuery(nextIdx >= 0 ? history[nextIdx] : savedQueryRef.current);
        return;
      }

      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setSelectedIndex((i) => Math.min(i + 1, items.length - 1));
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelectedIndex((i) => Math.max(i - 1, 0));
          break;
        case "Enter":
          e.preventDefault();
          if (items[selectedIndex]) {
            handleLaunch(items[selectedIndex]);
          }
          break;
        case "Escape":
          e.preventDefault();
          if (historyIndexRef.current >= 0) {
            historyIndexRef.current = -1;
            setQuery(savedQueryRef.current);
          } else if (query) {
            setQuery("");
          } else {
            getCurrentWindow().hide();
          }
          break;
        case ",":
          if (e.ctrlKey) {
            e.preventDefault();
            openSettings();
          }
          break;
      }
    },
    [query, results, selectedIndex, history, handleLaunch, openSettings]
  );

  // Resize the window to fit the actual rendered content. Uses
  // window.devicePixelRatio (not Rust's scale_factor) for physical pixel
  // conversion — they can disagree on some Windows DPI configurations.
  useLayoutEffect(() => {
    const el = contentRef.current;
    if (!el) return;
    const update = () => {
      if (!contentRef.current) return;
      const rect = contentRef.current.getBoundingClientRect();
      const cssH = Math.ceil(rect.height) + 2;
      const cols = settingsRef.current?.pinned_columns ?? pinnedColumns;
      if (cssH > 0 && cols > 0) {
        measuredHRef.current = cssH;
        api
          .setLauncherSize(cols, cssH, window.devicePixelRatio)
          .catch(() => {});
      }
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(el);
    return () => observer.disconnect();
  }, [pinnedColumns]);

  return (
    <div
      className="w-full h-full rounded-2xl overflow-hidden border border-white/20 dark:border-white/10 bg-white/90 dark:bg-gray-900/80 backdrop-blur-xl shadow-2xl"
      data-tauri-drag-region
    >
      {/* Toast */}
      {toast && (
        <div className="absolute top-3 left-1/2 -translate-x-1/2 z-50 px-4 py-2 bg-gray-800 dark:bg-gray-200 text-white dark:text-gray-900 text-sm rounded-lg shadow-lg">
          {toast}
        </div>
      )}

      {/* Drag overlay */}
      {dragOver && (
        <div className="absolute inset-0 z-40 bg-blue-500/10 backdrop-blur-sm flex items-center justify-center rounded-2xl pointer-events-none">
          <div className="px-6 py-4 bg-white dark:bg-gray-800 rounded-xl shadow-2xl border-2 border-dashed border-blue-400">
            <div className="text-sm font-medium text-gray-700 dark:text-gray-300">
              ドロップして登録
            </div>
          </div>
        </div>
      )}

      <div ref={contentRef}>
        <SearchBar
          value={query}
          onChange={(v) => { historyIndexRef.current = -1; setQuery(v); }}
          onKeyDown={handleKeyDown}
          onClear={() => { historyIndexRef.current = -1; setQuery(""); }}
        />
        <div className="border-t border-gray-200/50 dark:border-gray-700/50">
          {query.trim() ? (
            <ResultsList
              apps={results}
              selectedIndex={selectedIndex}
              onLaunch={handleLaunch}
              onExclude={handleExclude}
              onSetTags={handleSetTags}
              onPin={handlePin}
              pinnedIds={pinnedIds}
              maxPinned={pinnedColumns * pinnedRows}
              onSelect={setSelectedIndex}
            />
          ) : (
            <RecentStrip
              recentApps={recentApps}
              frequentApps={frequentApps}
              pinnedApps={pinnedApps}
              pinnedIds={pinnedIds}
              sections={sections}
              pinnedColumns={pinnedColumns}
              pinnedRows={pinnedRows}
              onLaunch={handleLaunch}
              onExclude={handleExclude}
              onSetTags={handleSetTags}
              onPin={handlePin}
              onUnpin={handleUnpin}
              onReorderPinned={handleReorderPinned}
            />
          )}
        </div>
      </div>
      <div className="absolute top-3 right-3 flex items-center gap-1">
        {showEvacButton && (
          <>
            {/* Evacuation toggle: ON copies the DB local so Dropbox can sync */}
            <button
              onClick={() => { if (!evacuated) handleEvacuate(); }}
              className={`flex items-center gap-1 px-1.5 py-1 rounded-lg text-[11px] transition-colors ${
                evacuated
                  ? "text-amber-600 dark:text-amber-400 bg-amber-500/10"
                  : "text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-500/10"
              }`}
              title={evacuated ? "退避中（ローカルDBを参照）" : "DBをローカルに退避してDropbox同期を解放"}
            >
              <span
                className={`relative w-7 h-4 rounded-full transition-colors ${
                  evacuated ? "bg-amber-500" : "bg-gray-300 dark:bg-gray-600"
                }`}
              >
                <span
                  className={`absolute top-0.5 left-0.5 w-3 h-3 bg-white rounded-full transition-transform shadow ${
                    evacuated ? "translate-x-3" : ""
                  }`}
                />
              </span>
              退避
            </button>
            {evacuated && (
              <>
                <button
                  onClick={handleMerge}
                  className="px-2 py-1 rounded-lg text-[11px] font-medium text-blue-600 dark:text-blue-400 hover:bg-blue-500/10 transition-colors"
                  title="ローカルの変更をDropboxへ統合して退避を解除"
                >
                  マージ実行
                </button>
                <button
                  onClick={handleDiscard}
                  className="px-2 py-1 rounded-lg text-[11px] text-gray-400 hover:text-red-500 hover:bg-red-500/10 transition-colors"
                  title="ローカルの変更を破棄して退避を解除"
                >
                  破棄
                </button>
              </>
            )}
            <div className="w-px h-4 bg-gray-300 dark:bg-gray-600 mx-0.5" />
          </>
        )}
        <button
          onClick={openSettings}
          className="p-1.5 rounded-lg text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-500/10 transition-colors"
          title="設定 (Ctrl+,)"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>
        <button
          onClick={() => getCurrentWindow().hide()}
          className="p-1.5 rounded-lg text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-500/10 transition-colors"
          title="閉じる (Esc)"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
    </div>
  );
}

export default LauncherWindow;
