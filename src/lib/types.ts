export interface AppEntry {
  id: string;
  name: string;
  exe_path: string;
  folder_id: string;
  icon_base64: string | null;
  tags: string[];
  launch_count: number;
  last_launched: number | null;
  is_excluded: boolean;
  is_renamed: boolean;
  created_at: number;
  item_type: string; // "app" | "file" | "folder"
}

export interface Folder {
  id: string;
  path: string;
  added_at: number;
  last_scanned: number | null;
}

export interface RegisteredItem {
  path: string;
  type: "app" | "file" | "folder";
  name?: string;
  tags?: string[];
}

export type SectionId = "recent" | "frequent" | "pinned";

export interface SectionConfig {
  id: SectionId;
  visible: boolean;
}

export type WindowPosition =
  | "center"
  | "top-left"
  | "top-right"
  | "bottom-left"
  | "bottom-right";

export interface Settings {
  hotkey: string;
  autostart: boolean;
  theme: "system" | "dark" | "light";
  max_results: number;
  file_extensions: string;
  registered_items: RegisteredItem[];
  pinned_items: string[];
  sections: SectionConfig[];
  pinned_columns: number;
  pinned_rows: number;
  debug_log: boolean;
  show_evac_button: boolean;
  /** Consecutive taps required to toggle the launcher (2 or 3). */
  hotkey_taps: number;
  /** Max gap between taps in milliseconds (200-800). */
  hotkey_interval_ms: number;
  window_position: WindowPosition;
  /** Margin from the screen edge in logical px (corner positions only). */
  window_margin: number;
  /** Create a daily backup on startup. */
  auto_backup: boolean;
  accent_color: "blue" | "green" | "purple" | "amber" | "rose" | "custom";
  /** Hex colour used when accent_color is "custom". */
  accent_custom: string;
  /** Absolute path to the launcher background image; empty = none. */
  background_image: string;
  /** Darkening over the background image, 0-100. */
  background_dim: number;
  /** Background image zoom, 100-300 (%). Above 100 crops to the window. */
  background_scale: number;
  /** Launcher window opacity, 50-100. 100 = fully opaque. */
  window_opacity: number;
  /** Launcher text colour. */
  text_color: "auto" | "light" | "dark" | "custom";
  /** Hex colour used when text_color is "custom". */
  text_custom: string;
}

export interface EvacStatus {
  evacuated: boolean;
}

export interface MergeResult {
  /** "overwrite" (Dropbox side untouched) or "merge" (3-way reconcile). */
  strategy: string;
  apps_added: number;
  apps_updated: number;
  folders_added: number;
}

export type ScanMode = "add_new" | "add_all_except_excluded";

