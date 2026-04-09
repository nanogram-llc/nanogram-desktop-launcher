import { useState, useEffect, useCallback } from "react";
import type { AppEntry } from "../../lib/types";
import * as api from "../../lib/tauri";

interface EditState {
  mode: "add" | "edit";
  id?: string;
  name: string;
  cmd: string;
  admin: boolean;
  keepOpen: boolean;
}

const EMPTY_EDIT: EditState = {
  mode: "add",
  name: "",
  cmd: "",
  admin: false,
  keepOpen: true,
};

function CommandManager() {
  const [items, setItems] = useState<AppEntry[]>([]);
  const [filter, setFilter] = useState("");
  const [toast, setToast] = useState<string | null>(null);
  const [edit, setEdit] = useState<EditState | null>(null);
  const [saving, setSaving] = useState(false);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3000);
  };

  const loadItems = useCallback(async () => {
    try {
      const result = await api.getItemsByType("command");
      setItems(result);
    } catch (e) {
      console.error("Failed to load commands:", e);
    }
  }, []);

  useEffect(() => {
    loadItems();
  }, [loadItems]);

  const openAdd = () => {
    setEdit({ ...EMPTY_EDIT });
  };

  const openEdit = async (item: AppEntry) => {
    try {
      const data = await api.getCommandData(item.id);
      setEdit({
        mode: "edit",
        id: item.id,
        name: item.name,
        cmd: data.cmd,
        admin: data.admin,
        keepOpen: data.keep_open,
      });
    } catch (e) {
      console.error("Failed to load command data:", e);
      showToast("コマンドの読み込みに失敗しました");
    }
  };

  const handleSave = async () => {
    if (!edit) return;
    if (!edit.name.trim() || !edit.cmd.trim()) {
      showToast("名前とコマンドを入力してください");
      return;
    }
    setSaving(true);
    try {
      if (edit.mode === "add") {
        await api.registerCommand(
          edit.name.trim(),
          edit.cmd,
          edit.admin,
          edit.keepOpen
        );
        showToast("コマンドを登録しました");
      } else if (edit.id) {
        await api.updateCommand(edit.id, edit.cmd, edit.admin, edit.keepOpen);
        // If name changed, also rename
        const original = items.find((i) => i.id === edit.id);
        if (original && original.name !== edit.name.trim()) {
          await api.renameApp(edit.id, edit.name.trim());
        }
        showToast("コマンドを更新しました");
      }
      setEdit(null);
      await loadItems();
    } catch (e) {
      console.error("Failed to save command:", e);
      showToast("保存に失敗しました");
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("このコマンドを削除しますか?")) return;
    try {
      await api.deleteApp(id);
      setItems((prev) => prev.filter((a) => a.id !== id));
      showToast("削除しました");
    } catch (e) {
      console.error("Failed to delete:", e);
    }
  };

  const handleRun = async (id: string) => {
    try {
      await api.launchCommand(id);
    } catch (e) {
      console.error("Failed to run command:", e);
      showToast(`実行失敗: ${e}`);
    }
  };

  const filteredItems = items.filter((item) => {
    if (!filter.trim()) return true;
    const q = filter.toLowerCase();
    return (
      item.name.toLowerCase().includes(q) ||
      item.tags.some((t) => t.toLowerCase().includes(q))
    );
  });

  return (
    <div className="space-y-4 relative">
      {toast && (
        <div className="fixed top-3 right-3 z-50 px-4 py-2 bg-gray-800 dark:bg-gray-200 text-white dark:text-gray-900 text-sm rounded-lg shadow-lg animate-fade-in">
          {toast}
        </div>
      )}

      {/* Search + add */}
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="コマンドを検索..."
          className="flex-1 px-3 py-2 text-sm bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 outline-none focus:border-blue-400"
        />
        <button
          onClick={openAdd}
          className="px-3 py-2 text-xs font-medium bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors whitespace-nowrap"
        >
          + 追加
        </button>
      </div>

      {/* Empty state */}
      {items.length === 0 && !filter && (
        <div className="border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-xl py-8 text-center">
          <svg
            className="w-10 h-10 mx-auto mb-2 text-gray-400"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
            />
          </svg>
          <div className="text-sm text-gray-500 dark:text-gray-400">
            「+ 追加」ボタンからコマンドを登録
          </div>
        </div>
      )}

      {/* Item list */}
      <div className="space-y-1 max-h-[420px] overflow-y-auto">
        {filteredItems.length === 0 && (items.length > 0 || filter) ? (
          <div className="text-sm text-gray-500 dark:text-gray-400 py-8 text-center">
            {filter
              ? "該当するコマンドが見つかりません"
              : "コマンドが登録されていません"}
          </div>
        ) : (
          filteredItems.map((item) => (
            <CommandRow
              key={item.id}
              item={item}
              onRun={() => handleRun(item.id)}
              onEdit={() => openEdit(item)}
              onDelete={() => handleDelete(item.id)}
            />
          ))
        )}
      </div>

      {/* Edit dialog */}
      {edit && (
        <div
          className="fixed inset-0 z-50 bg-black/30 backdrop-blur-sm flex items-center justify-center p-4"
          onClick={() => !saving && setEdit(null)}
        >
          <div
            className="w-full max-w-xl bg-white dark:bg-gray-800 rounded-2xl shadow-2xl border border-gray-200 dark:border-gray-700 p-5 space-y-4"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="text-base font-semibold text-gray-900 dark:text-gray-100">
              {edit.mode === "add" ? "コマンドを追加" : "コマンドを編集"}
            </div>

            <div className="space-y-3">
              <div>
                <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
                  名前
                </label>
                <input
                  type="text"
                  value={edit.name}
                  onChange={(e) => setEdit({ ...edit, name: e.target.value })}
                  placeholder="例: プロジェクトをビルド"
                  className="w-full px-3 py-2 text-sm bg-gray-100 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg text-gray-900 dark:text-gray-100 outline-none focus:border-blue-400"
                  autoFocus
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
                  コマンド（1行1コマンド、空行・<code>::</code> / <code>REM</code> で始まる行はスキップ）
                </label>
                <textarea
                  value={edit.cmd}
                  onChange={(e) => setEdit({ ...edit, cmd: e.target.value })}
                  placeholder={"cd C:\\path\\to\\project\nnpm run build"}
                  rows={8}
                  spellCheck={false}
                  className="w-full px-3 py-2 text-sm font-mono bg-gray-100 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg text-gray-900 dark:text-gray-100 outline-none focus:border-blue-400 resize-y"
                />
                <div className="text-[10px] text-gray-500 dark:text-gray-500 mt-1">
                  内部では <code>cmd.exe</code> で <code>&amp;&amp;</code> 区切りで順次実行されます
                </div>
              </div>

              <div className="flex items-center gap-4 pt-1">
                <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={edit.admin}
                    onChange={(e) =>
                      setEdit({ ...edit, admin: e.target.checked })
                    }
                    className="rounded accent-blue-500"
                  />
                  管理者として実行
                </label>
                <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={edit.keepOpen}
                    onChange={(e) =>
                      setEdit({ ...edit, keepOpen: e.target.checked })
                    }
                    className="rounded accent-blue-500"
                  />
                  実行後もウィンドウを閉じない
                </label>
              </div>
            </div>

            <div className="flex justify-end gap-2 pt-2">
              <button
                onClick={() => setEdit(null)}
                disabled={saving}
                className="px-4 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors disabled:opacity-50"
              >
                キャンセル
              </button>
              <button
                onClick={handleSave}
                disabled={saving}
                className="px-4 py-2 text-sm font-medium bg-blue-500 text-white rounded-lg hover:bg-blue-600 transition-colors disabled:opacity-50"
              >
                {saving ? "保存中..." : "保存"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

interface CommandRowProps {
  item: AppEntry;
  onRun: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

function CommandRow({ item, onRun, onEdit, onDelete }: CommandRowProps) {
  return (
    <div className="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors group">
      <div className="w-8 h-8 rounded-md bg-gray-200 dark:bg-gray-700 flex items-center justify-center shrink-0">
        <svg
          className="w-4 h-4 text-gray-600 dark:text-gray-300"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
          />
        </svg>
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
          {item.name}
        </div>
        <div className="text-[10px] text-gray-400 dark:text-gray-500">
          {item.launch_count > 0 ? `${item.launch_count}回実行` : "未実行"}
        </div>
      </div>
      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
        <button
          onClick={onRun}
          className="px-2 py-1 text-xs bg-green-500 text-white rounded hover:bg-green-600"
          title="実行"
        >
          実行
        </button>
        <button
          onClick={onEdit}
          className="px-2 py-1 text-xs bg-gray-500 text-white rounded hover:bg-gray-600"
          title="編集"
        >
          編集
        </button>
        <button
          onClick={onDelete}
          className="px-2 py-1 text-xs bg-red-500 text-white rounded hover:bg-red-600"
          title="削除"
        >
          削除
        </button>
      </div>
    </div>
  );
}

export default CommandManager;
