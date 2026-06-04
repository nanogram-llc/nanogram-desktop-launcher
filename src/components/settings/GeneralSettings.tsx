import { useState, useEffect, useRef, useCallback } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import type { Settings, SectionConfig, SectionId } from "../../lib/types";
import * as api from "../../lib/tauri";

const DEFAULT_SECTIONS: SectionConfig[] = [
  { id: "recent", visible: true },
  { id: "frequent", visible: true },
  { id: "pinned", visible: true },
];

const SECTION_LABELS: Record<SectionId, string> = {
  recent: "最近使った",
  frequent: "よく使う",
  pinned: "固定",
};

interface GeneralSettingsProps {
  onThemeChange: (theme: string) => void;
}

/** Convert a KeyboardEvent to a shortcut string like "Ctrl+Space" */
function eventToShortcut(e: KeyboardEvent): string | null {
  // Ignore modifier-only presses
  if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return null;

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");

  // Need at least one modifier
  if (parts.length === 0) return null;

  // Normalize key name
  let key = e.key;
  if (key === " ") key = "Space";
  else if (key === "Enter") key = "Return";
  else if (key.length === 1) key = key.toUpperCase();

  parts.push(key);
  return parts.join("+");
}

/** Display-friendly label for a shortcut */
function shortcutLabel(hotkey: string): string {
  if (!hotkey.includes("+")) {
    const map: Record<string, string> = {
      Control: "Ctrl × 2",
      Alt: "Alt × 2",
      Shift: "Shift × 2",
    };
    return map[hotkey] || hotkey;
  }
  return hotkey
    .replace("Ctrl", "Ctrl")
    .replace("Return", "Enter");
}

/**
 * 列数/行数変更時に pinned_items を2次元座標で再マッピングする。
 * 各アイテムの (row, col) 位置を保持し、新しいサイズに収まらない
 * スロット（col >= newCols または row >= newRows）は空文字に落とす。
 * これにより行をまたぐ位置ジャンプを防ぐ。
 */
function remapPinnedItems(
  items: string[],
  oldCols: number,
  oldRows: number,
  newCols: number,
  newRows: number,
): string[] {
  const result: string[] = [];
  for (let r = 0; r < newRows; r++) {
    for (let c = 0; c < newCols; c++) {
      const id =
        r < oldRows && c < oldCols ? (items[r * oldCols + c] ?? "") : "";
      result.push(id);
    }
  }
  return result;
}

function GeneralSettings({ onThemeChange }: GeneralSettingsProps) {
  const [settings, setSettings] = useState<Settings>({
    hotkey: "Control",
    autostart: false,
    theme: "system",
    max_results: 8,
    file_extensions: "txt,json,md,csv,html,xml,pdf,docx,xlsx,pptx,doc,xls,ppt",
    registered_items: [],
    pinned_items: [],
    sections: DEFAULT_SECTIONS,
    pinned_columns: 9,
    pinned_rows: 2,
    debug_log: false,
    show_evac_button: false,
  });
  const [loaded, setLoaded] = useState(false);
  const [recording, setRecording] = useState(false);
  const [regenStatus, setRegenStatus] = useState<string | null>(null);
  const [evacuated, setEvacuated] = useState(false);
  const [evacStatus, setEvacStatus] = useState<string | null>(null);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleRegenAllIcons = useCallback(async () => {
    setRegenStatus("再生成中...");
    try {
      const n = await api.regenerateAllIcons();
      setRegenStatus(`${n} 件のアイコンを再生成しました`);
    } catch (e) {
      setRegenStatus("再生成に失敗しました");
      console.error("regenerateAllIcons failed:", e);
    }
    setTimeout(() => setRegenStatus(null), 4000);
  }, []);

  const isComboMode = settings.hotkey.includes("+");

  useEffect(() => {
    api.getSettings().then((s) => {
      if (!s.sections || s.sections.length === 0) {
        s.sections = DEFAULT_SECTIONS;
      }
      setSettings(s);
      setLoaded(true);
    }).catch(() => setLoaded(true));
  }, []);

  // Evacuation status (machine-local) — fetch on mount, keep in sync.
  useEffect(() => {
    api.getEvacStatus().then((s) => setEvacuated(s.evacuated)).catch(() => {});
    const unlisten = listen<{ evacuated: boolean }>("evac-changed", (e) => {
      setEvacuated(e.payload.evacuated);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const flashEvac = useCallback((msg: string) => {
    setEvacStatus(msg);
    setTimeout(() => setEvacStatus(null), 4000);
  }, []);

  const handleEvacuate = useCallback(async () => {
    try {
      const s = await api.evacuate();
      setEvacuated(s.evacuated);
      flashEvac("DBをローカルに退避しました");
    } catch (e) {
      flashEvac(`退避に失敗: ${e}`);
    }
  }, [flashEvac]);

  const handleMerge = useCallback(async () => {
    try {
      const r = await api.mergeEvacuation();
      setEvacuated(false);
      flashEvac(
        r.strategy === "overwrite"
          ? "Dropboxへ反映しました（上書き）"
          : `Dropboxへマージ（追加 ${r.apps_added} / 更新 ${r.apps_updated} / フォルダ ${r.folders_added}）`
      );
    } catch (e) {
      flashEvac(`マージに失敗: ${e}`);
    }
  }, [flashEvac]);

  const handleDiscard = useCallback(async () => {
    try {
      const s = await api.discardEvacuation();
      setEvacuated(s.evacuated);
      flashEvac("退避を破棄しました");
    } catch (e) {
      flashEvac(`破棄に失敗: ${e}`);
    }
  }, [flashEvac]);

  const autoSave = useCallback((newSettings: Settings) => {
    emit("settings-changed", newSettings).catch(console.error);
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => {
      api.updateSettings(newSettings).catch((e) =>
        console.error("Failed to save settings:", e)
      );
    }, 300);
  }, []);

  const update = useCallback((patch: Partial<Settings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...patch };
      autoSave(next);
      return next;
    });
  }, [autoSave]);

  // Key recording listener
  useEffect(() => {
    if (!recording) return;

    const handler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const shortcut = eventToShortcut(e);
      if (shortcut) {
        setRecording(false);
        update({ hotkey: shortcut });
      }
    };

    // Also cancel on Escape
    const escHandler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setRecording(false);
      }
    };

    window.addEventListener("keydown", handler, true);
    window.addEventListener("keydown", escHandler);
    return () => {
      window.removeEventListener("keydown", handler, true);
      window.removeEventListener("keydown", escHandler);
    };
  }, [recording, update]);

  if (!loaded) return null;

  return (
    <div className="space-y-6">
      <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">一般設定</h2>

      <div className="space-y-4">
        {/* Autostart */}
        <label className="flex items-center justify-between">
          <div>
            <div className="text-sm font-medium text-gray-700 dark:text-gray-300">起動時に自動実行</div>
            <div className="text-xs text-gray-500 dark:text-gray-400">システム起動時に自動的に起動</div>
          </div>
          <button
            onClick={() => update({ autostart: !settings.autostart })}
            className={`relative w-11 h-6 rounded-full transition-colors ${
              settings.autostart ? "bg-blue-500" : "bg-gray-300 dark:bg-gray-600"
            }`}
          >
            <span
              className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform shadow ${
                settings.autostart ? "translate-x-5" : ""
              }`}
            />
          </button>
        </label>

        {/* Theme */}
        <div>
          <div className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">テーマ</div>
          <div className="flex gap-2">
            {(["system", "light", "dark"] as const).map((t) => (
              <button
                key={t}
                onClick={() => {
                  update({ theme: t });
                  onThemeChange(t);
                }}
                className={`px-4 py-2 text-sm rounded-lg border transition-colors ${
                  settings.theme === t
                    ? "bg-blue-500 text-white border-blue-500"
                    : "bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-600 hover:border-blue-400"
                }`}
              >
                {t === "system" ? "システム" : t === "light" ? "ライト" : "ダーク"}
              </button>
            ))}
          </div>
        </div>

        {/* Max Results */}
        <div>
          <div className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            検索結果の最大表示件数: {settings.max_results}
          </div>
          <input
            type="range"
            min={4}
            max={20}
            value={settings.max_results}
            onChange={(e) => update({ max_results: parseInt(e.target.value) })}
            className="w-full accent-blue-500"
          />
        </div>

        {/* Pinned Columns */}
        <div>
          <div className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            固定エリア 列数: {settings.pinned_columns}
          </div>
          <input
            type="range"
            min={5}
            max={15}
            value={settings.pinned_columns}
            onChange={(e) => {
              const newCols = parseInt(e.target.value);
              const newItems = remapPinnedItems(
                settings.pinned_items,
                settings.pinned_columns,
                settings.pinned_rows,
                newCols,
                settings.pinned_rows,
              );
              const patch = { pinned_columns: newCols, pinned_items: newItems };
              update(patch);
              // ウィンドウが閉じる前にdebounceが発火しない場合に備えて即座にディスク保存
              api.updateSettings({ ...settings, ...patch }).catch(console.error);
            }}
            className="w-full accent-blue-500"
          />
        </div>

        {/* Pinned Rows */}
        <div>
          <div className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            固定エリア 行数: {settings.pinned_rows}
          </div>
          <input
            type="range"
            min={1}
            max={5}
            value={settings.pinned_rows}
            onChange={(e) => {
              const newRows = parseInt(e.target.value);
              const newItems = remapPinnedItems(
                settings.pinned_items,
                settings.pinned_columns,
                settings.pinned_rows,
                settings.pinned_columns,
                newRows,
              );
              const patch = { pinned_rows: newRows, pinned_items: newItems };
              update(patch);
              // ウィンドウが閉じる前にdebounceが発火しない場合に備えて即座にディスク保存
              api.updateSettings({ ...settings, ...patch }).catch(console.error);
            }}
            className="w-full accent-blue-500"
          />
        </div>

        {/* File Extensions */}
        <div>
          <div className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">登録可能なファイル拡張子</div>
          <div className="text-xs text-gray-500 dark:text-gray-400 mb-2">
            カンマ区切りで拡張子を指定（ドロップ時にフィルタリング）
          </div>
          <input
            type="text"
            value={settings.file_extensions}
            onChange={(e) => update({ file_extensions: e.target.value })}
            placeholder="txt,json,md,csv,pdf,docx,xlsx"
            className="w-full px-3 py-2 text-sm bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 outline-none focus:border-blue-400"
          />
        </div>

        {/* Section display & order */}
        <div>
          <div className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">表示セクション</div>
          <div className="text-xs text-gray-500 dark:text-gray-400 mb-2">
            表示/非表示を切り替え、矢印で順序を変更
          </div>
          <div className="space-y-1">
            {(settings.sections || DEFAULT_SECTIONS).map((sec, idx) => {
              const secs = settings.sections || DEFAULT_SECTIONS;
              const canUp = idx > 0;
              const canDown = idx < secs.length - 1;
              const moveSection = (from: number, to: number) => {
                const newSections = [...secs];
                const [moved] = newSections.splice(from, 1);
                newSections.splice(to, 0, moved);
                update({ sections: newSections });
              };
              return (
                <div
                  key={sec.id}
                  className="flex items-center gap-2 px-3 py-2 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50"
                >
                  <div className="flex flex-col gap-0.5 shrink-0">
                    <button
                      onClick={() => canUp && moveSection(idx, idx - 1)}
                      disabled={!canUp}
                      className={`p-0.5 rounded ${canUp ? "text-gray-500 hover:text-gray-700 dark:hover:text-gray-300" : "text-gray-300 dark:text-gray-700"}`}
                    >
                      <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
                      </svg>
                    </button>
                    <button
                      onClick={() => canDown && moveSection(idx, idx + 1)}
                      disabled={!canDown}
                      className={`p-0.5 rounded ${canDown ? "text-gray-500 hover:text-gray-700 dark:hover:text-gray-300" : "text-gray-300 dark:text-gray-700"}`}
                    >
                      <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                      </svg>
                    </button>
                  </div>
                  <span className="text-sm text-gray-700 dark:text-gray-300 flex-1">
                    {SECTION_LABELS[sec.id as SectionId] || sec.id}
                  </span>
                  <button
                    onClick={() => {
                      const newSections = (settings.sections || DEFAULT_SECTIONS).map((s, i) =>
                        i === idx ? { ...s, visible: !s.visible } : s
                      );
                      update({ sections: newSections });
                    }}
                    className={`relative w-9 h-5 rounded-full transition-colors ${
                      sec.visible ? "bg-blue-500" : "bg-gray-300 dark:bg-gray-600"
                    }`}
                  >
                    <span
                      className={`absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform shadow ${
                        sec.visible ? "translate-x-4" : ""
                      }`}
                    />
                  </button>
                </div>
              );
            })}
          </div>
        </div>

        {/* Hotkey */}
        <div>
          <div className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">ホットキー</div>

          {/* Mode selector */}
          <div className="flex gap-2 mb-3">
            <button
              onClick={() => {
                if (isComboMode) update({ hotkey: "Control" });
              }}
              className={`px-3 py-1.5 text-xs rounded-lg border transition-colors ${
                !isComboMode
                  ? "bg-blue-500 text-white border-blue-500"
                  : "bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-600 hover:border-blue-400"
              }`}
            >
              ダブルタップ
            </button>
            <button
              onClick={() => {
                if (!isComboMode) {
                  setRecording(true);
                }
              }}
              className={`px-3 py-1.5 text-xs rounded-lg border transition-colors ${
                isComboMode
                  ? "bg-blue-500 text-white border-blue-500"
                  : "bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-600 hover:border-blue-400"
              }`}
            >
              キー組み合わせ
            </button>
          </div>

          {/* Double-tap options */}
          {!isComboMode && !recording && (
            <div>
              <div className="text-xs text-gray-500 dark:text-gray-400 mb-2">
                選択したキーを素早く2回押すとランチャーが表示/非表示されます
              </div>
              <div className="flex gap-2">
                {([
                  { value: "Control", label: "Ctrl × 2" },
                  { value: "Alt", label: "Alt × 2" },
                  { value: "Shift", label: "Shift × 2" },
                ] as const).map((opt) => (
                  <button
                    key={opt.value}
                    onClick={() => update({ hotkey: opt.value })}
                    className={`px-4 py-2 text-sm rounded-lg border transition-colors ${
                      settings.hotkey === opt.value
                        ? "bg-blue-500 text-white border-blue-500"
                        : "bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 border-gray-300 dark:border-gray-600 hover:border-blue-400"
                    }`}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Combo mode display */}
          {isComboMode && !recording && (
            <div>
              <div className="text-xs text-gray-500 dark:text-gray-400 mb-2">
                設定されたキー組み合わせでランチャーが表示/非表示されます
              </div>
              <div className="flex items-center gap-3">
                <div className="px-4 py-2 text-sm font-mono font-medium bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-gray-100">
                  {shortcutLabel(settings.hotkey)}
                </div>
                <button
                  onClick={() => setRecording(true)}
                  className="px-3 py-2 text-xs font-medium bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors"
                >
                  変更
                </button>
              </div>
            </div>
          )}

          {/* Recording state */}
          {recording && (
            <div className="flex items-center gap-3">
              <div className="px-4 py-3 text-sm bg-yellow-50 dark:bg-yellow-900/20 border-2 border-yellow-400 dark:border-yellow-600 rounded-lg text-yellow-700 dark:text-yellow-300 animate-pulse">
                キーを押してください... (Escでキャンセル)
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Maintenance */}
      <div className="border-t border-gray-200 dark:border-gray-700 pt-4 space-y-2">
        <div className="text-sm font-medium text-gray-700 dark:text-gray-300">メンテナンス</div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleRegenAllIcons}
            disabled={regenStatus === "再生成中..."}
            className="px-3 py-1.5 text-xs font-medium bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-300 dark:hover:bg-gray-600 transition-colors disabled:opacity-50"
          >
            アイコンを再生成
          </button>
          {regenStatus && (
            <span className="text-xs text-gray-500 dark:text-gray-400">{regenStatus}</span>
          )}
        </div>
        <div className="text-[10px] text-gray-400 dark:text-gray-500">
          欠けているアイコンや古いアイコンを最新の抽出ロジックで再取得します（数十秒かかる場合あり）
        </div>
      </div>

      {/* DB evacuation mode */}
      <div className="border-t border-gray-200 dark:border-gray-700 pt-4 space-y-3">
        <div>
          <div className="text-sm font-medium text-gray-700 dark:text-gray-300">DB退避モード</div>
          <div className="text-[11px] text-gray-500 dark:text-gray-400 mt-1 leading-relaxed">
            DB をEXE隣（Dropbox同期フォルダ）からローカルに退避し、同期の競合（つっかえ）を防ぎます。
            退避中はローカルDBを参照し、Dropbox側は触らないので同期が完了します。
            作業後「マージ実行」で変更をDropboxへ戻し、退避を解除します（Dropbox側が他PCで更新されていた場合は自動で統合）。
            「破棄」はローカルの変更を捨てて退避を解除します。
          </div>
        </div>

        <div className="flex items-center gap-2">
          <span
            className={`px-2 py-0.5 rounded-full text-[11px] font-medium ${
              evacuated
                ? "bg-amber-500/15 text-amber-600 dark:text-amber-400"
                : "bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400"
            }`}
          >
            {evacuated ? "退避中（ローカルDB）" : "通常（Dropbox DB）"}
          </span>
          {!evacuated ? (
            <button
              onClick={handleEvacuate}
              className="px-3 py-1.5 text-xs font-medium bg-amber-500/90 text-white rounded-lg hover:bg-amber-500 transition-colors"
            >
              ローカルに退避
            </button>
          ) : (
            <>
              <button
                onClick={handleMerge}
                className="px-3 py-1.5 text-xs font-medium bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors"
              >
                マージ実行
              </button>
              <button
                onClick={handleDiscard}
                className="px-3 py-1.5 text-xs font-medium bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-red-500/10 hover:text-red-500 transition-colors"
              >
                破棄
              </button>
            </>
          )}
          {evacStatus && (
            <span className="text-xs text-gray-500 dark:text-gray-400">{evacStatus}</span>
          )}
        </div>

        {/* Show controls on the main launcher window */}
        <label className="flex items-center justify-between pt-1">
          <div>
            <div className="text-sm font-medium text-gray-700 dark:text-gray-300">退避操作をメイン画面に表示</div>
            <div className="text-xs text-gray-500 dark:text-gray-400">ランチャー右上の設定ボタン左に退避ボタンを表示</div>
          </div>
          <button
            onClick={() => update({ show_evac_button: !settings.show_evac_button })}
            className={`relative w-11 h-6 rounded-full transition-colors ${
              settings.show_evac_button ? "bg-blue-500" : "bg-gray-300 dark:bg-gray-600"
            }`}
          >
            <span
              className={`absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform shadow ${
                settings.show_evac_button ? "translate-x-5" : ""
              }`}
            />
          </button>
        </label>
      </div>

      <div className="flex items-center justify-between">
        <div className="text-xs text-gray-400 dark:text-gray-500">
          設定は自動的に保存されます
        </div>
        <button
          onClick={() => api.openSettingsFile().catch(() => {})}
          className="text-xs text-blue-500 hover:text-blue-600 dark:text-blue-400 dark:hover:text-blue-300"
        >
          settings.json を開く
        </button>
      </div>
    </div>
  );
}

export default GeneralSettings;
