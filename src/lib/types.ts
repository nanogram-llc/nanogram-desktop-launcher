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

export interface Settings {
  hotkey: string;
  autostart: boolean;
  theme: "system" | "dark" | "light";
  max_results: number;
  recent_count: number;
  frequent_count: number;
  file_extensions: string;
  registered_items: RegisteredItem[];
  pinned_items: string[];
  sections: SectionConfig[];
  pinned_columns: number;
  pinned_rows: number;
  debug_log: boolean;
}

export type ScanMode = "add_new" | "add_all_except_excluded";

