import { invoke } from "@tauri-apps/api/core";
import type { AppEntry, Folder, Settings, ScanMode, EvacStatus, MergeResult } from "./types";

export async function getApps(
  query: string | null,
  limit: number
): Promise<AppEntry[]> {
  return invoke("get_apps", { query, limit });
}

export async function launchApp(appId: string): Promise<void> {
  return invoke("launch_app", { appId });
}

export async function launchAppAsAdmin(appId: string): Promise<void> {
  return invoke("launch_app_as_admin", { appId });
}

export async function deleteApp(appId: string): Promise<void> {
  return invoke("delete_app", { appId });
}

export async function renameApp(
  appId: string,
  newName: string
): Promise<void> {
  return invoke("rename_app", { appId, newName });
}

export async function setTags(
  appId: string,
  tags: string[]
): Promise<void> {
  return invoke("set_tags", { appId, tags });
}

export async function excludeApp(appId: string): Promise<void> {
  return invoke("exclude_app", { appId });
}

export async function includeApp(appId: string): Promise<void> {
  return invoke("include_app", { appId });
}

export interface AddFolderResult {
  folder: Folder;
  app_count: number;
}

export async function addFolder(path: string): Promise<AddFolderResult> {
  return invoke("add_folder", { path });
}

export async function setBlurHide(enabled: boolean): Promise<void> {
  return invoke("set_blur_hide", { enabled });
}

export async function resizeWindow(cols: number, totalRows: number, sectionCount: number): Promise<void> {
  return invoke("resize_window", { cols, totalRows, sectionCount });
}

/// Resize the launcher window using JS-reported devicePixelRatio for
/// physical pixel conversion (the only DPR that matches the WebView's
/// actual rendering). cssHeight is in CSS pixels.
export async function setLauncherSize(
  cols: number,
  cssHeight: number,
  dpr: number
): Promise<void> {
  return invoke("set_launcher_size", { cols, height: cssHeight, dpr });
}

export async function removeFolder(folderId: string): Promise<void> {
  return invoke("remove_folder", { folderId });
}

export async function getFolders(): Promise<Folder[]> {
  return invoke("get_folders");
}

export async function rescanFolder(
  folderId: string,
  mode: ScanMode
): Promise<AppEntry[]> {
  return invoke("rescan_folder", { folderId, mode });
}

export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export async function updateSettings(settings: Settings): Promise<void> {
  return invoke("update_settings", { settings });
}

// ── DB evacuation mode ──

export async function getEvacStatus(): Promise<EvacStatus> {
  return invoke("get_evac_status");
}

export async function evacuate(): Promise<EvacStatus> {
  return invoke("evacuate");
}

export async function mergeEvacuation(): Promise<MergeResult> {
  return invoke("merge_evacuation");
}

export async function discardEvacuation(): Promise<EvacStatus> {
  return invoke("discard_evacuation");
}

export async function getIcon(appId: string): Promise<string> {
  return invoke("get_icon", { appId });
}

export interface StartMenuPaths {
  all_users: string | null;
  current_user: string | null;
}

export async function getStartmenuPaths(): Promise<StartMenuPaths> {
  return invoke("get_startmenu_paths");
}

export async function getExcludedApps(): Promise<AppEntry[]> {
  return invoke("get_excluded_apps");
}

export async function bulkExcludeApps(appIds: string[]): Promise<void> {
  return invoke("bulk_exclude_apps", { appIds });
}

export async function bulkIncludeApps(appIds: string[]): Promise<void> {
  return invoke("bulk_include_apps", { appIds });
}

export async function registerFiles(paths: string[]): Promise<number> {
  return invoke("register_files", { paths });
}

export async function registerItems(paths: string[], itemType: string): Promise<number> {
  return invoke("register_items", { paths, itemType });
}

export async function getItemsByType(itemType: string): Promise<AppEntry[]> {
  return invoke("get_items_by_type", { itemType });
}

export async function getExcludedItemsByType(itemType: string): Promise<AppEntry[]> {
  return invoke("get_excluded_items_by_type", { itemType });
}

export async function openItem(appId: string): Promise<void> {
  return invoke("open_item", { appId });
}

export async function openFolderWith(folderPath: string, program: string): Promise<void> {
  return invoke("open_folder_with", { folderPath, program });
}

export async function openSettingsFile(): Promise<void> {
  return invoke("open_settings_file");
}

export async function getExeDir(): Promise<string> {
  return invoke("get_exe_dir");
}

export async function setVibracyTheme(dark: boolean): Promise<void> {
  return invoke("set_vibrancy_theme", { dark });
}

// ── Command items ──

export type Shell = "cmd" | "pwsh";

export interface CommandData {
  cmd: string;
  admin: boolean;
  keep_open: boolean;
  shell?: Shell;
}

export async function registerCommand(
  name: string,
  cmd: string,
  admin: boolean,
  keepOpen: boolean,
  shell: Shell = "cmd"
): Promise<string> {
  return invoke("register_command", { name, cmd, admin, keepOpen, shell });
}

export async function updateCommand(
  appId: string,
  cmd: string,
  admin: boolean,
  keepOpen: boolean,
  shell: Shell = "cmd"
): Promise<void> {
  return invoke("update_command", { appId, cmd, admin, keepOpen, shell });
}

export async function regenerateMissingIcons(): Promise<number> {
  return invoke("regenerate_missing_icons");
}

export async function regenerateAllIcons(): Promise<number> {
  return invoke("regenerate_all_icons");
}

export async function getCommandData(appId: string): Promise<CommandData> {
  return invoke("get_command_data", { appId });
}

export async function launchCommand(appId: string): Promise<void> {
  return invoke("launch_command", { appId });
}
