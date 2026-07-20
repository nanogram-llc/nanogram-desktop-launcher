import { useState, useEffect, useCallback } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import type { AppEntry } from "../../lib/types";
import * as api from "../../lib/tauri";
import { addPinnedItem, getPinState } from "../../lib/pin";
import AppRow from "./AppRow";

interface ItemTableProps {
  itemType: "file" | "folder";
  label: string;
  dropHint: string;
  allowedExtensions?: string;
}

function ItemTable({ itemType, label, dropHint }: ItemTableProps) {
  const [items, setItems] = useState<AppEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [toast, setToast] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [pinnedIds, setPinnedIds] = useState<Set<string>>(new Set());
  const [pinFull, setPinFull] = useState(false);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3000);
  };

  const loadItems = useCallback(async () => {
    try {
      const result = await api.getItemsByType(itemType);
      setItems(result);
    } catch (e) {
      console.error(`Failed to load ${itemType}s:`, e);
    }
  }, [itemType]);

  const loadPinState = useCallback(async () => {
    try {
      const { pinned, full } = await getPinState();
      setPinnedIds(pinned);
      setPinFull(full);
    } catch (e) {
      console.error("Failed to load pin state:", e);
    }
  }, []);

  useEffect(() => {
    loadItems();
    loadPinState();
  }, [loadItems, loadPinState]);

  // 固定状態は他ウィンドウ（ランチャー）からの変更でも更新する
  useEffect(() => {
    const unlisten = listen("settings-changed", () => {
      loadPinState();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loadPinState]);

  const handlePin = useCallback(async (id: string) => {
    try {
      const result = await addPinnedItem(id);
      if (result === "added") {
        showToast("固定エリアに追加しました");
      } else if (result === "already") {
        showToast("すでに固定済みです");
      } else {
        showToast("固定エリアが満杯です");
      }
      await loadPinState();
    } catch (e) {
      console.error("Failed to pin item:", e);
      showToast("固定に失敗しました");
    }
  }, [loadPinState]);

  // Drag & drop
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

        try {
          const count = await api.registerItems(paths, itemType);
          if (count > 0) {
            showToast(`${count} 件の${label}を登録しました`);
            await loadItems();
          } else {
            showToast(`新規の${label}はありませんでした`);
          }
        } catch (e) {
          console.error("Failed to register dropped items:", e);
          showToast("登録に失敗しました");
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [itemType, label, loadItems]);

  const filteredItems = items.filter((item) => {
    if (!filter.trim()) return true;
    const q = filter.toLowerCase();
    return (
      item.name.toLowerCase().includes(q) ||
      item.tags.some((t) => t.toLowerCase().includes(q)) ||
      item.exe_path.toLowerCase().includes(q)
    );
  });

  const toggleSelect = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleSelectAll = () => {
    if (selected.size === filteredItems.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(filteredItems.map((a) => a.id)));
    }
  };

  const handleRename = async (id: string, newName: string) => {
    try {
      await api.renameApp(id, newName);
      setItems((prev) =>
        prev.map((a) => (a.id === id ? { ...a, name: newName, is_renamed: true } : a))
      );
    } catch (e) {
      console.error("Failed to rename:", e);
    }
  };

  const handleSetTags = async (id: string, tags: string[]) => {
    try {
      await api.setTags(id, tags);
      setItems((prev) =>
        prev.map((a) => (a.id === id ? { ...a, tags } : a))
      );
    } catch (e) {
      console.error("Failed to set tags:", e);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await api.deleteApp(id);
      setItems((prev) => prev.filter((a) => a.id !== id));
      setSelected((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    } catch (e) {
      console.error("Failed to delete:", e);
    }
  };

  const handleLaunch = async (id: string) => {
    try {
      await api.openItem(id);
    } catch (e) {
      console.error("Failed to open item:", e);
    }
  };

  const handleBulkDelete = async () => {
    if (selected.size === 0) return;
    const ids = Array.from(selected);
    try {
      for (const id of ids) {
        await api.deleteApp(id);
      }
      setItems((prev) => prev.filter((a) => !selected.has(a.id)));
      showToast(`${ids.length} 件を削除しました`);
      setSelected(new Set());
    } catch (e) {
      console.error("Failed to bulk delete:", e);
    }
  };

  return (
    <div className="space-y-4 relative">
      {toast && (
        <div className="fixed top-3 right-3 z-50 px-4 py-2 bg-gray-800 dark:bg-gray-200 text-white dark:text-gray-900 text-sm rounded-lg shadow-lg animate-fade-in">
          {toast}
        </div>
      )}

      {dragOver && (
        <div className="fixed inset-0 z-40 accent-soft-bg backdrop-blur-sm flex items-center justify-center pointer-events-none">
          <div className="px-8 py-6 bg-white dark:bg-gray-800 rounded-2xl shadow-2xl border-2 border-dashed accent-border">
            <div className="text-center">
              <svg className="w-12 h-12 mx-auto mb-3 accent-text" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 4v16m8-8H4" />
              </svg>
              <div className="text-sm font-medium text-gray-700 dark:text-gray-300">
                {dropHint}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Search + bulk */}
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder={`${label}を検索...`}
          className="flex-1 px-3 py-2 text-sm bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 outline-none accent-focus"
        />
        {selected.size > 0 && (
          <button
            onClick={handleBulkDelete}
            className="px-3 py-2 text-xs font-medium bg-red-500 text-white rounded-lg hover:bg-red-600 transition-colors whitespace-nowrap"
          >
            {selected.size}件を削除
          </button>
        )}
      </div>

      {/* Select all */}
      {filteredItems.length > 0 && (
        <label className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400 cursor-pointer select-none">
          <input
            type="checkbox"
            checked={selected.size === filteredItems.length && filteredItems.length > 0}
            onChange={toggleSelectAll}
            className="rounded border-gray-300 dark:border-gray-600 accent-input"
          />
          すべて選択 ({filteredItems.length}件)
        </label>
      )}

      {/* Empty state */}
      {items.length === 0 && !filter && (
        <div className="border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-xl py-8 text-center">
          <svg className="w-10 h-10 mx-auto mb-2 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 4v16m8-8H4" />
          </svg>
          <div className="text-sm text-gray-500 dark:text-gray-400">{dropHint}</div>
        </div>
      )}

      {/* Item list */}
      <div className="space-y-1 max-h-[360px] overflow-y-auto">
        {filteredItems.length === 0 && (items.length > 0 || filter) ? (
          <div className="text-sm text-gray-500 dark:text-gray-400 py-8 text-center">
            {filter
              ? `該当する${label}が見つかりません`
              : `${label}が登録されていません`}
          </div>
        ) : (
          filteredItems.map((item) => (
            <AppRow
              key={item.id}
              app={item}
              selected={selected.has(item.id)}
              onToggleSelect={toggleSelect}
              onRename={handleRename}
              onSetTags={handleSetTags}
              onRemove={handleDelete}
              onLaunch={handleLaunch}
              removeMode="delete"
              onPin={handlePin}
              isPinned={pinnedIds.has(item.id)}
              pinFull={pinFull}
            />
          ))
        )}
      </div>
    </div>
  );
}

export default ItemTable;
