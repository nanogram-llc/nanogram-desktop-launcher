import { useState, useEffect, useRef, useCallback } from "react";
import { emit } from "@tauri-apps/api/event";
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
  });
  const [loaded, setLoaded] = useState(false);
  const [recording, setRecording] = useState(false);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

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

  const autoSave = useCallback((newSettings: Settings) => {
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => {
      api.updateSettings(newSettings).then(() => {
        emit("settings-changed", newSettings).catch(console.error);
      }).catch((e) =>
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
            onChange={(e) => update({ pinned_columns: parseInt(e.target.value) })}
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
            onChange={(e) => update({ pinned_rows: parseInt(e.target.value) })}
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
