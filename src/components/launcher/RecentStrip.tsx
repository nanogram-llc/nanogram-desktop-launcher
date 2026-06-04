import { useState, useEffect, useLayoutEffect, useRef, useCallback } from "react";
import type { AppEntry, SectionConfig } from "../../lib/types";
import * as api from "../../lib/tauri";
import CommandIcon, { parseCommandExePath } from "./CommandIcon";

interface RecentStripProps {
  recentApps: AppEntry[];
  frequentApps: AppEntry[];
  pinnedApps: AppEntry[];
  pinnedIds: string[];
  sections: SectionConfig[];
  pinnedColumns: number;
  pinnedRows: number;
  onLaunch: (app: AppEntry) => void;
  onExclude: (app: AppEntry) => void;
  onSetTags: (app: AppEntry, tags: string[]) => void;
  onPin: (appId: string) => void;
  onUnpin: (appId: string) => void;
  onReorderPinned: (newIds: string[]) => void;
}

function AppIcon({
  app,
  onLaunch,
  onContextMenu,
  onMouseDown,
  dimmed,
}: {
  app: AppEntry;
  onLaunch: (app: AppEntry) => void;
  onContextMenu: (e: React.MouseEvent, app: AppEntry) => void;
  onMouseDown?: (e: React.MouseEvent) => void;
  dimmed?: boolean;
}) {
  return (
    <button
      onClick={() => onLaunch(app)}
      onContextMenu={(e) => onContextMenu(e, app)}
      onMouseDown={onMouseDown}
      className={`flex flex-col items-center gap-1 p-1.5 rounded-xl hover:bg-gray-500/10 dark:hover:bg-gray-400/10 transition-colors duration-100 w-full select-none ${
        dimmed ? "opacity-30" : ""
      }`}
      title={app.name}
    >
      {app.item_type === "command" ? (
        <CommandIcon shell={parseCommandExePath(app.exe_path)?.shell ?? "cmd"} />
      ) : app.icon_base64 ? (
        <img src={app.icon_base64} alt="" className="w-10 h-10 rounded-lg pointer-events-none" draggable={false} />
      ) : (
        <div className="w-10 h-10 rounded-lg bg-gray-300 dark:bg-gray-600 flex items-center justify-center">
          <svg className="w-5 h-5 text-gray-500 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2z" />
          </svg>
        </div>
      )}
      <span className="text-[10px] text-gray-600 dark:text-gray-400 truncate w-full text-center leading-tight">
        {app.name}
      </span>
    </button>
  );
}


function RecentStrip({
  recentApps,
  frequentApps,
  pinnedApps,
  pinnedIds,
  sections,
  pinnedColumns,
  pinnedRows,
  onLaunch,
  onExclude,
  onSetTags,
  onPin,
  onUnpin,
  onReorderPinned,
}: RecentStripProps) {
  const [menuState, setMenuState] = useState<{ x: number; y: number; app: AppEntry; isPinned: boolean; ctrlKey: boolean } | null>(null);
  const [editingTags, setEditingTags] = useState<AppEntry | null>(null);
  const [tagInput, setTagInput] = useState("");
  const menuRef = useRef<HTMLDivElement>(null);

  const gridSlots = pinnedColumns * pinnedRows;

  // Custom mouse-based drag state
  const [dragSourceIdx, setDragSourceIdx] = useState<number | null>(null);
  const [dragOverIdx, setDragOverIdx] = useState<number | null>(null);
  const [ghostPos, setGhostPos] = useState<{ x: number; y: number } | null>(null);
  const isDragging = useRef(false);
  const dragStartPos = useRef<{ x: number; y: number } | null>(null);
  const pendingSourceIdx = useRef<number | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const slotRefs = useRef<(HTMLDivElement | null)[]>([]);

  // Build grid
  const appMap = new Map(pinnedApps.map((a) => [a.id, a]));
  const grid: (AppEntry | null)[] = [];
  for (let i = 0; i < gridSlots; i++) {
    const id = pinnedIds[i] || "";
    grid.push(id ? appMap.get(id) || null : null);
  }
  const filledCount = pinnedIds.slice(0, gridSlots).filter((id) => id !== "").length;
  const isPinnedFull = filledCount >= gridSlots;

  // Find which slot the mouse is over
  const getSlotAtPos = useCallback((clientX: number, clientY: number): number | null => {
    for (let i = 0; i < slotRefs.current.length; i++) {
      const el = slotRefs.current[i];
      if (!el) continue;
      const rect = el.getBoundingClientRect();
      if (clientX >= rect.left && clientX <= rect.right && clientY >= rect.top && clientY <= rect.bottom) {
        return i;
      }
    }
    return null;
  }, []);

  // Mouse drag handlers
  const handleSlotMouseDown = useCallback((e: React.MouseEvent, slotIdx: number) => {
    if (e.button !== 0) return; // left click only
    dragStartPos.current = { x: e.clientX, y: e.clientY };
    pendingSourceIdx.current = slotIdx;
  }, []);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      // Check if we should start dragging (5px threshold)
      if (!isDragging.current && pendingSourceIdx.current !== null && dragStartPos.current) {
        const dx = e.clientX - dragStartPos.current.x;
        const dy = e.clientY - dragStartPos.current.y;
        if (Math.abs(dx) > 5 || Math.abs(dy) > 5) {
          isDragging.current = true;
          setDragSourceIdx(pendingSourceIdx.current);
        }
      }

      if (!isDragging.current) return;

      e.preventDefault();
      setGhostPos({ x: e.clientX, y: e.clientY });

      const slot = getSlotAtPos(e.clientX, e.clientY);
      setDragOverIdx(slot !== null && slot !== pendingSourceIdx.current ? slot : null);
    };

    const handleMouseUp = (e: MouseEvent) => {
      if (isDragging.current && pendingSourceIdx.current !== null) {
        const targetIdx = getSlotAtPos(e.clientX, e.clientY);
        if (targetIdx !== null && targetIdx !== pendingSourceIdx.current) {
          // Swap/move
          const newIds = [...pinnedIds];
          while (newIds.length <= Math.max(pendingSourceIdx.current, targetIdx)) {
            newIds.push("");
          }
          const temp = newIds[pendingSourceIdx.current];
          newIds[pendingSourceIdx.current] = newIds[targetIdx];
          newIds[targetIdx] = temp;
          onReorderPinned(newIds);
        }
      }

      // Reset all drag state
      isDragging.current = false;
      dragStartPos.current = null;
      pendingSourceIdx.current = null;
      setDragSourceIdx(null);
      setDragOverIdx(null);
      setGhostPos(null);
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [pinnedIds, onReorderPinned, getSlotAtPos]);

  useEffect(() => {
    if (!menuState) return;
    const handler = () => setMenuState(null);
    window.addEventListener("click", handler);
    return () => window.removeEventListener("click", handler);
  }, [menuState]);

  // Adjust menu position before paint so it never overflows the viewport
  useLayoutEffect(() => {
    if (!menuState || !menuRef.current) return;
    const el = menuRef.current;
    const rect = el.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let x = menuState.x;
    let y = menuState.y;
    if (x + rect.width > vw) x = Math.max(0, x - rect.width);
    if (y + rect.height > vh) y = Math.max(0, y - rect.height);
    el.style.left = `${x}px`;
    el.style.top = `${y}px`;
  }, [menuState]);

  const handleContextMenu = (e: React.MouseEvent, app: AppEntry, isPinned: boolean) => {
    e.preventDefault();
    e.stopPropagation();
    setMenuState({ x: e.clientX, y: e.clientY, app, isPinned, ctrlKey: e.ctrlKey });
  };

  const handleSaveTags = () => {
    if (!editingTags) return;
    const tags = tagInput
      .split(",")
      .map((t) => t.trim())
      .filter((t) => t.length > 0);
    onSetTags(editingTags, tags);
    setEditingTags(null);
  };

  const isEmpty = recentApps.length === 0 && frequentApps.length === 0 && filledCount === 0;

  if (isEmpty) {
    return (
      <div className="px-5 py-8 text-center text-sm text-gray-400 dark:text-gray-500">
        フォルダを追加してアプリを登録してください
      </div>
    );
  }

  // Ghost element: the app being dragged
  const draggedApp = dragSourceIdx !== null ? grid[dragSourceIdx] : null;

  const SECTION_LABELS: Record<string, string> = {
    recent: "最近使った",
    frequent: "よく使う",
    pinned: "固定",
  };

  // 1fr で列幅を等分 — 固定px だと DPI 差で overflow して欠けるため
  const gridStyle = { gridTemplateColumns: `repeat(${pinnedColumns}, minmax(0, 1fr))` };

  const renderSection = (secId: string) => {
    if (secId === "recent" && recentApps.length > 0) {
      return (
        <div key={secId} className="mb-2">
          <div className="text-[10px] font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider px-1 mb-1">
            {SECTION_LABELS[secId]}
          </div>
          <div className="grid gap-0.5 w-full" style={gridStyle}>
            {recentApps.slice(0, pinnedColumns).map((app) => (
              <div key={app.id}>
                <AppIcon app={app} onLaunch={onLaunch} onContextMenu={(e, a) => handleContextMenu(e, a, false)} />
              </div>
            ))}
          </div>
        </div>
      );
    }
    if (secId === "frequent" && frequentApps.length > 0) {
      return (
        <div key={secId} className="mb-2">
          <div className="text-[10px] font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider px-1 mb-1">
            {SECTION_LABELS[secId]}
          </div>
          <div className="grid gap-0.5 w-full" style={gridStyle}>
            {frequentApps.slice(0, pinnedColumns).map((app) => (
              <div key={app.id}>
                <AppIcon app={app} onLaunch={onLaunch} onContextMenu={(e, a) => handleContextMenu(e, a, false)} />
              </div>
            ))}
          </div>
        </div>
      );
    }
    if (secId === "pinned") {
      return (
        <div key={secId}>
          <div className="text-[10px] font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider px-1 mb-1">
            {SECTION_LABELS[secId]}
          </div>
          <div
            ref={gridRef}
            className="grid gap-0.5 w-full"
            style={gridStyle}
          >
            {grid.map((app, idx) => {
              const isSource = dragSourceIdx === idx;
              const isOver = dragOverIdx === idx;
              return (
                <div
                  key={`slot-${idx}`}
                  ref={(el) => { slotRefs.current[idx] = el; }}
                  className={`rounded-xl transition-all duration-100 ${
                    isOver ? "ring-2 ring-blue-400 bg-blue-500/10" : ""
                  } ${!app ? "aspect-square bg-gray-100/50 dark:bg-gray-800/30" : ""}`}
                >
                  {app && (
                    <AppIcon
                      app={app}
                      onLaunch={isDragging.current ? () => {} : onLaunch}
                      onContextMenu={(e, a) => handleContextMenu(e, a, true)}
                      onMouseDown={(e) => handleSlotMouseDown(e, idx)}
                      dimmed={isSource}
                    />
                  )}
                </div>
              );
            })}
          </div>
        </div>
      );
    }
    return null;
  };

  return (
    <div className="px-4 pt-2 pb-5 relative">
      {sections.filter((s) => s.visible).map((s) => renderSection(s.id))}

      {/* Drag ghost */}
      {draggedApp && ghostPos && (
        <div
          className="fixed z-[100] pointer-events-none opacity-80"
          style={{ left: ghostPos.x - 32, top: ghostPos.y - 32 }}
        >
          {draggedApp.icon_base64 ? (
            <img src={draggedApp.icon_base64} alt="" className="w-16 h-16 rounded-lg shadow-lg" />
          ) : (
            <div className="w-16 h-16 rounded-lg bg-gray-400 shadow-lg" />
          )}
        </div>
      )}

      {/* Inline tag editor */}
      {editingTags && (
        <div className="absolute left-4 right-4 bottom-2 flex gap-2 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg p-2 shadow-lg z-50">
          <input
            type="text"
            value={tagInput}
            onChange={(e) => setTagInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSaveTags();
              if (e.key === "Escape") setEditingTags(null);
              e.stopPropagation();
            }}
            placeholder="タグをカンマ区切りで入力"
            className="flex-1 px-2 py-1 text-xs bg-gray-100 dark:bg-gray-700 rounded border border-gray-300 dark:border-gray-600 outline-none text-gray-900 dark:text-gray-100 focus:border-blue-400"
            autoFocus
          />
          <button
            onClick={handleSaveTags}
            className="px-2 py-1 text-xs bg-blue-500 text-white rounded hover:bg-blue-600"
          >
            保存
          </button>
          <button
            onClick={() => setEditingTags(null)}
            className="px-2 py-1 text-xs bg-gray-200 dark:bg-gray-600 text-gray-700 dark:text-gray-300 rounded hover:bg-gray-300 dark:hover:bg-gray-500"
          >
            ×
          </button>
        </div>
      )}

      {/* Context menu */}
      {menuState && (
        <div
          ref={menuRef}
          className="fixed z-50 py-1 min-w-[140px] bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-xl"
          style={{ left: menuState.x, top: menuState.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="px-3 py-1 text-[10px] text-gray-400 dark:text-gray-500 truncate border-b border-gray-100 dark:border-gray-700">
            {menuState.app.name}
          </div>
          {(menuState.app.item_type === "app" || menuState.app.item_type == null || menuState.app.item_type === "file") && (
            <>
              <button
                className="w-full px-3 py-1.5 text-xs text-left text-amber-600 dark:text-amber-400 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors font-medium"
                onClick={() => {
                  const app = menuState.app;
                  setMenuState(null);
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
              const app = menuState.app;
              setMenuState(null);
              setTagInput(app.tags.join(", "));
              setEditingTags(app);
            }}
          >
            タグ編集
          </button>
          {menuState.isPinned ? (
            <button
              className="w-full px-3 py-1.5 text-xs text-left text-orange-600 dark:text-orange-400 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
              onClick={() => {
                const app = menuState.app;
                setMenuState(null);
                onUnpin(app.id);
              }}
            >
              固定を解除
            </button>
          ) : (
            <button
              className={`w-full px-3 py-1.5 text-xs text-left hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors ${
                isPinnedFull || pinnedIds.includes(menuState.app.id)
                  ? "text-gray-400 dark:text-gray-600 cursor-not-allowed"
                  : "text-blue-600 dark:text-blue-400"
              }`}
              disabled={isPinnedFull || pinnedIds.includes(menuState.app.id)}
              onClick={() => {
                const app = menuState.app;
                setMenuState(null);
                onPin(app.id);
              }}
            >
              {pinnedIds.includes(menuState.app.id) ? "固定済み" : isPinnedFull ? "固定エリア満杯" : "固定エリアに追加"}
            </button>
          )}
          <button
            className="w-full px-3 py-1.5 text-xs text-left text-red-600 dark:text-red-400 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
            onClick={() => {
              const app = menuState.app;
              setMenuState(null);
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

export default RecentStrip;
