import { emit } from "@tauri-apps/api/event";
import * as api from "./tauri";

export type PinResult = "added" | "already" | "full";

/**
 * 設定ファイルの pinned_items に appId を追加する共有ロジック。
 * 設定ウィンドウ（管理画面）からの固定登録に使用する。
 *
 * グリッドサイズ（pinned_columns × pinned_rows）に正規化し、最初の空き
 * スロットに追加する。保存後に "settings-changed" を emit し、開いている
 * ランチャーウィンドウへ即時反映させる（ランチャー側は listen で受信）。
 */
export async function addPinnedItem(appId: string): Promise<PinResult> {
  const s = await api.getSettings();
  const cols = s.pinned_columns || 9;
  const rows = s.pinned_rows || 2;
  const slots = cols * rows;
  const next = Array.from({ length: slots }, (_, i) => s.pinned_items?.[i] ?? "");
  if (next.includes(appId)) return "already";
  const emptyIdx = next.indexOf("");
  if (emptyIdx === -1) return "full";
  next[emptyIdx] = appId;
  const updated = { ...s, pinned_items: next };
  await api.updateSettings(updated);
  await emit("settings-changed", updated);
  return "added";
}

/** 現在固定中のアイテムID集合と、グリッドが満杯かを取得する。 */
export async function getPinState(): Promise<{ pinned: Set<string>; full: boolean }> {
  const s = await api.getSettings();
  const cols = s.pinned_columns || 9;
  const rows = s.pinned_rows || 2;
  const slots = cols * rows;
  const ids = (s.pinned_items ?? []).slice(0, slots).filter((id) => id !== "");
  return { pinned: new Set(ids), full: ids.length >= slots };
}
