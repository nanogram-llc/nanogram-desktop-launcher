import { useState, useRef, useEffect, useLayoutEffect } from "react";
import type { AppEntry } from "../../lib/types";
import * as api from "../../lib/tauri";
import CommandIcon, { parseCommandExePath } from "./CommandIcon";

interface ResultItemProps {
  app: AppEntry;
  isSelected: boolean;
  onLaunch: (app: AppEntry) => void;
  onExclude: (app: AppEntry) => void;
  onSetTags: (app: AppEntry, tags: string[]) => void;
  onPin: (appId: string) => void;
  pinnedIds: string[];
  maxPinned: number;
  onMouseEnter: () => void;
}

function ResultItem({ app, isSelected, onLaunch, onExclude, onSetTags, onPin, pinnedIds, maxPinned, onMouseEnter }: ResultItemProps) {
  const isPinnedFull = pinnedIds.filter((id) => id !== "").length >= maxPinned;
  const isAlreadyPinned = pinnedIds.includes(app.id);
  const [menuPos, setMenuPos] = useState<{ x: number; y: number } | null>(null);
  const [editingTags, setEditingTags] = useState(false);
  const [tagInput, setTagInput] = useState("");
  const menuRef = useRef<HTMLDivElement>(null);

  // Close menu on outside click
  useEffect(() => {
    if (!menuPos) return;
    const handler = () => setMenuPos(null);
    window.addEventListener("click", handler);
    return () => window.removeEventListener("click", handler);
  }, [menuPos]);

  // Adjust menu position before paint so it never overflows the viewport
  useLayoutEffect(() => {
    if (!menuPos || !menuRef.current) return;
    const el = menuRef.current;
    const rect = el.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let x = menuPos.x;
    let y = menuPos.y;
    if (x + rect.width > vw) x = Math.max(0, x - rect.width);
    if (y + rect.height > vh) y = Math.max(0, y - rect.height);
    el.style.left = `${x}px`;
    el.style.top = `${y}px`;
  }, [menuPos]);

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setMenuPos({ x: e.clientX, y: e.clientY });
  };

  const handleSaveTags = () => {
    const tags = tagInput
      .split(",")
      .map((t) => t.trim())
      .filter((t) => t.length > 0);
    onSetTags(app, tags);
    setEditingTags(false);
  };

  const isCommand = app.item_type === "command";
  const cmdData = isCommand ? parseCommandExePath(app.exe_path) : null;

  const displayPath = (() => {
    if (!isCommand) return app.exe_path;
    if (!cmdData) return "(command)";
    const firstLine = cmdData.cmd
      .split("\n")
      .map((l: string) => l.trim())
      .find((l: string) => l.length > 0);
    const prefix = cmdData.admin ? "⚡ " : "";
    return prefix + (firstLine ? `${firstLine} …` : "(command)");
  })();

  return (
    <div
      className={`px-5 py-2.5 cursor-pointer transition-colors duration-100 relative
        ${isSelected ? "accent-soft-bg" : "hover:bg-gray-500/10 dark:hover:bg-gray-400/10"}`}
      onClick={() => !editingTags && onLaunch(app)}
      onContextMenu={handleContextMenu}
      onMouseEnter={onMouseEnter}
    >
      <div className="flex items-center gap-3">
        {isCommand ? (
          <CommandIcon shell={cmdData?.shell ?? "cmd"} size="sm" />
        ) : app.icon_base64 ? (
          <img
            src={app.icon_base64}
            alt=""
            className="w-8 h-8 rounded-md shrink-0"
          />
        ) : (
          <div className="w-8 h-8 rounded-md bg-gray-300 dark:bg-gray-600 flex items-center justify-center shrink-0">
            <svg className="w-4 h-4 text-gray-500 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2z" />
            </svg>
          </div>
        )}
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
            {app.name}
          </div>
          <div className="text-[10px] text-gray-400 dark:text-gray-500 truncate" title={displayPath}>
            {displayPath}
          </div>
          {app.tags.length > 0 && !editingTags && (
            <div className="flex gap-1 mt-0.5">
              {app.tags.slice(0, 3).map((tag) => (
                <span
                  key={tag}
                  className="text-[10px] px-1.5 py-0.5 rounded-full bg-gray-200/80 dark:bg-gray-700/80 text-gray-600 dark:text-gray-400"
                >
                  {tag}
                </span>
              ))}
            </div>
          )}
        </div>
        {app.launch_count > 0 && !editingTags && (
          <span className="text-xs text-gray-400 dark:text-gray-500 shrink-0">
            {app.launch_count}回
          </span>
        )}
      </div>

      {/* Inline tag editor */}
      {editingTags && (
        <div
          className="flex gap-2 mt-1.5 ml-11"
          onClick={(e) => e.stopPropagation()}
        >
          <input
            type="text"
            value={tagInput}
            onChange={(e) => setTagInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSaveTags();
              if (e.key === "Escape") setEditingTags(false);
              e.stopPropagation();
            }}
            placeholder="タグをカンマ区切りで入力"
            className="flex-1 px-2 py-1 text-xs bg-gray-100 dark:bg-gray-700 rounded border border-gray-300 dark:border-gray-600 outline-none text-gray-900 dark:text-gray-100 accent-focus"
            autoFocus
          />
          <button
            onClick={handleSaveTags}
            className="px-2 py-1 text-xs accent-bg text-white rounded"
          >
            保存
          </button>
        </div>
      )}

      {/* Context menu */}
      {menuPos && (
        <div
          ref={menuRef}
          className="fixed z-50 py-1 min-w-[120px] bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-xl"
          style={{ left: menuPos.x, top: menuPos.y }}
          onClick={(e) => e.stopPropagation()}
        >
          {(app.item_type === "app" || app.item_type == null || app.item_type === "file") && (
            <>
              <button
                className="w-full px-3 py-1.5 text-xs text-left text-amber-600 dark:text-amber-400 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors font-medium"
                onClick={() => {
                  setMenuPos(null);
                  api.launchAppAsAdmin(app.id).catch(console.error);
                }}
              >
                🛡 管理者として実行
              </button>
              <div className="my-1 border-t border-gray-200 dark:border-gray-700" />
            </>
          )}
          <button
            className="w-full px-3 py-1.5 text-xs text-left text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
            onClick={() => {
              setMenuPos(null);
              setTagInput(app.tags.join(", "));
              setEditingTags(true);
            }}
          >
            タグ編集
          </button>
          {(app.item_type === "app" || app.item_type == null || app.item_type === "file") && (
            <button
              className="w-full px-3 py-1.5 text-xs text-left text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
              onClick={() => {
                setMenuPos(null);
                api.revealInExplorer(app.id).catch(console.error);
              }}
            >
              アプリの場所を開く
            </button>
          )}
          {app.item_type === "folder" && (
            <>
              <button
                className="w-full px-3 py-1.5 text-xs text-left text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                onClick={() => {
                  setMenuPos(null);
                  api.openFolderWith(app.exe_path, "explorer").catch(console.error);
                }}
              >
                エクスプローラーで開く
              </button>
              <button
                className="w-full px-3 py-1.5 text-xs text-left text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                onClick={() => {
                  setMenuPos(null);
                  api.openFolderWith(app.exe_path, "wt").catch(() => {
                    api.openFolderWith(app.exe_path, "cmd").catch(console.error);
                  });
                }}
              >
                ターミナルで開く
              </button>
            </>
          )}
          <button
            className={`w-full px-3 py-1.5 text-xs text-left hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors ${
              isPinnedFull || isAlreadyPinned
                ? "text-gray-400 dark:text-gray-600 cursor-not-allowed"
                : "accent-text"
            }`}
            disabled={isPinnedFull || isAlreadyPinned}
            onClick={() => {
              setMenuPos(null);
              onPin(app.id);
            }}
          >
            {isAlreadyPinned ? "固定済み" : isPinnedFull ? "固定エリア満杯" : "固定エリアに追加"}
          </button>
          <button
            className="w-full px-3 py-1.5 text-xs text-left text-red-600 dark:text-red-400 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
            onClick={() => {
              setMenuPos(null);
              onExclude(app);
            }}
          >
            除外
          </button>
        </div>
      )}
    </div>
  );
}

export default ResultItem;
