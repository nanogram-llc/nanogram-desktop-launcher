import { useState } from "react";
import type { AppEntry } from "../../lib/types";

interface AppRowProps {
  app: AppEntry;
  selected: boolean;
  onToggleSelect: (appId: string) => void;
  onRename: (appId: string, newName: string) => void;
  onSetTags: (appId: string, tags: string[]) => void;
  onRemove: (appId: string) => void;
  onLaunch: (appId: string) => void;
  /** "exclude" = hide icon, "include" = eye icon, "delete" = trash icon */
  removeMode: "exclude" | "include" | "delete";
  /** 指定時、固定エリア追加ボタンを表示する */
  onPin?: (appId: string) => void;
  /** 既に固定済みか */
  isPinned?: boolean;
  /** 固定エリアが満杯か */
  pinFull?: boolean;
}

function AppRow({
  app,
  selected,
  onToggleSelect,
  onRename,
  onSetTags,
  onRemove,
  onLaunch,
  removeMode,
  onPin,
  isPinned,
  pinFull,
}: AppRowProps) {
  const [editing, setEditing] = useState(false);
  const [editName, setEditName] = useState(app.name);
  const [editingTags, setEditingTags] = useState(false);
  const [tagInput, setTagInput] = useState(app.tags.join(", "));

  const handleSaveName = () => {
    if (editName.trim() && editName !== app.name) {
      onRename(app.id, editName.trim());
    }
    setEditing(false);
  };

  const handleSaveTags = () => {
    const tags = tagInput
      .split(",")
      .map((t) => t.trim())
      .filter((t) => t.length > 0);
    onSetTags(app.id, tags);
    setEditingTags(false);
  };

  return (
    <div
      className={`p-3 rounded-lg border transition-colors ${
        selected
          ? "accent-soft-bg accent-border"
          : removeMode === "include"
            ? "bg-gray-100/50 dark:bg-gray-800/30 border-gray-200/30 dark:border-gray-700/30"
            : "bg-white dark:bg-gray-800/80 border-gray-200/50 dark:border-gray-700/50"
      }`}
    >
      <div className="flex items-center gap-2">
        {/* Checkbox */}
        <input
          type="checkbox"
          checked={selected}
          onChange={() => onToggleSelect(app.id)}
          className="rounded border-gray-300 dark:border-gray-600 accent-input shrink-0"
        />

        {/* Icon */}
        {app.icon_base64 ? (
          <img src={app.icon_base64} alt="" className="w-8 h-8 rounded-md shrink-0" />
        ) : (
          <div className="w-8 h-8 rounded-md bg-gray-200 dark:bg-gray-700 shrink-0" />
        )}

        {/* Name */}
        <div className="flex-1 min-w-0">
          {editing ? (
            <input
              type="text"
              value={editName}
              onChange={(e) => setEditName(e.target.value)}
              onBlur={handleSaveName}
              onKeyDown={(e) => e.key === "Enter" && handleSaveName()}
              className="w-full px-2 py-0.5 text-sm bg-gray-100 dark:bg-gray-700 rounded border accent-border outline-none text-gray-900 dark:text-gray-100"
              autoFocus
            />
          ) : (
            <div
              className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate cursor-pointer hover:accent-text"
              onClick={() => {
                setEditName(app.name);
                setEditing(true);
              }}
              title="クリックで名前を変更"
            >
              {app.name}
            </div>
          )}
          <div className="text-[10px] text-gray-400 dark:text-gray-500 truncate" title={app.exe_path}>
            {app.exe_path}
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-1 shrink-0">
          {/* Pin to pinned area */}
          {onPin && (
            <button
              onClick={() => onPin(app.id)}
              disabled={isPinned || pinFull}
              className={`p-1 transition-colors ${
                isPinned
                  ? "accent-text cursor-default"
                  : pinFull
                    ? "text-gray-300 dark:text-gray-600 cursor-not-allowed"
                    : "text-gray-400 hover:accent-text"
              }`}
              title={isPinned ? "固定済み" : pinFull ? "固定エリアが満杯です" : "固定エリアに追加"}
            >
              <svg className="w-3.5 h-3.5" fill={isPinned ? "currentColor" : "none"} stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
              </svg>
            </button>
          )}

          {/* Launch */}
          {removeMode !== "include" && (
            <button
              onClick={() => onLaunch(app.id)}
              className="p-1 text-gray-400 hover:text-green-500 transition-colors"
              title="起動"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </button>
          )}

          {/* Tags */}
          <button
            onClick={() => {
              setTagInput(app.tags.join(", "));
              setEditingTags(!editingTags);
            }}
            className="p-1 text-gray-400 hover:accent-text transition-colors"
            title="タグ編集"
          >
            <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
            </svg>
          </button>

          {/* Remove action */}
          {removeMode === "exclude" && (
            <button
              onClick={() => onRemove(app.id)}
              className="p-1 text-gray-400 hover:text-yellow-500 transition-colors"
              title="除外"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.878 9.878L3 3m6.878 6.878L21 21" />
              </svg>
            </button>
          )}
          {removeMode === "include" && (
            <button
              onClick={() => onRemove(app.id)}
              className="p-1 text-gray-400 hover:text-green-500 transition-colors"
              title="復旧"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
              </svg>
            </button>
          )}
          {removeMode === "delete" && (
            <button
              onClick={() => onRemove(app.id)}
              className="p-1 text-gray-400 hover:text-red-500 transition-colors"
              title="削除"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          )}
        </div>
      </div>

      {/* Tags editing */}
      {editingTags && (
        <div className="mt-2 flex gap-2 ml-7">
          <input
            type="text"
            value={tagInput}
            onChange={(e) => setTagInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSaveTags()}
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

      {/* Existing tags display */}
      {!editingTags && app.tags.length > 0 && (
        <div className="flex gap-1 mt-1.5 ml-12">
          {app.tags.map((tag) => (
            <span
              key={tag}
              className="text-[10px] px-1.5 py-0.5 rounded-full bg-blue-100/80 dark:bg-blue-900/30 accent-text"
            >
              {tag}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

export default AppRow;
