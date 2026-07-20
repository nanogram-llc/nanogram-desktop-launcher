use serde::{Deserialize, Serialize};

/// An item defined in settings.json for auto-registration on startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RegisteredItem {
    /// Simple form: just a path string (defaults to type "app")
    Simple(String),
    /// Full form with optional name/tags/type
    Full {
        path: String,
        #[serde(rename = "type", default = "default_item_type")]
        item_type: String,
        name: Option<String>,
        tags: Option<Vec<String>>,
    },
}

fn default_item_type() -> String {
    "app".to_string()
}

impl RegisteredItem {
    pub fn path(&self) -> &str {
        match self {
            RegisteredItem::Simple(p) => p,
            RegisteredItem::Full { path, .. } => path,
        }
    }
    pub fn item_type(&self) -> &str {
        match self {
            RegisteredItem::Simple(_) => "app",
            RegisteredItem::Full { item_type, .. } => item_type,
        }
    }
    pub fn name(&self) -> Option<&str> {
        match self {
            RegisteredItem::Simple(_) => None,
            RegisteredItem::Full { name, .. } => name.as_deref(),
        }
    }
    pub fn tags(&self) -> Vec<String> {
        match self {
            RegisteredItem::Simple(_) => Vec::new(),
            RegisteredItem::Full { tags, .. } => tags.clone().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub exe_path: String,
    pub folder_id: String,
    pub icon_cache_path: Option<String>,
    pub tags: Vec<String>,
    pub launch_count: i64,
    pub last_launched: Option<i64>,
    pub is_excluded: bool,
    pub is_renamed: bool,
    pub created_at: i64,
    /// "app", "file", or "folder"
    pub item_type: String,
    #[serde(skip_deserializing)]
    pub icon_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub path: String,
    pub added_at: i64,
    pub last_scanned: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionConfig {
    pub id: String,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hotkey: String,
    pub autostart: bool,
    pub theme: String,
    pub max_results: u32,
    pub file_extensions: String,
    pub registered_items: Vec<RegisteredItem>,
    pub pinned_items: Vec<String>,
    pub sections: Vec<SectionConfig>,
    pub pinned_columns: u32,
    pub pinned_rows: u32,
    pub excluded_patterns: Vec<String>,
    pub debug_log: bool,
    /// Show the DB evacuation controls on the main launcher window.
    pub show_evac_button: bool,
    /// Number of consecutive taps required to toggle the launcher (2 or 3).
    pub hotkey_taps: u32,
    /// Max gap between taps, in milliseconds (200-800).
    pub hotkey_interval_ms: u32,
    /// Where the launcher appears on the target monitor:
    /// "center" | "top-left" | "top-right" | "bottom-left" | "bottom-right"
    pub window_position: String,
    /// Margin from the screen edge in logical pixels (corner positions only).
    pub window_margin: u32,
    /// Create a daily backup on startup (kept separately from manual ones).
    pub auto_backup: bool,
    /// Accent colour preset: "blue"|"green"|"purple"|"amber"|"rose"|"custom"
    pub accent_color: String,
    /// Hex colour ("#3b82f6") used when `accent_color` is "custom".
    pub accent_custom: String,
    /// Absolute path to the launcher background image; empty = none.
    pub background_image: String,
    /// Darkening applied over the background image, 0-100.
    pub background_dim: u32,
    /// Background image zoom, 100-300 (%). Above 100 the image is cropped by
    /// the window, letting the user frame a detail.
    pub background_scale: u32,
    /// How far the background image bleeds past the launcher card, in logical
    /// pixels (0-200). The OS window is grown by this much on every side and the
    /// surrounding area left transparent, so the image can extend beyond the
    /// card's rounded frame.
    pub background_overflow: u32,
    /// Launcher window opacity, 50-100. 100 = fully opaque.
    pub window_opacity: u32,
    /// Launcher text colour: "auto" | "light" | "dark" | "custom"
    pub text_color: String,
    /// Hex colour used when `text_color` is "custom".
    pub text_custom: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "Control".to_string(),
            autostart: false,
            theme: "system".to_string(),
            max_results: 8,
            file_extensions: "txt,json,md,csv,html,xml,pdf,docx,xlsx,pptx,doc,xls,ppt".to_string(),
            registered_items: Vec::new(),
            pinned_items: Vec::new(),
            sections: vec![
                SectionConfig { id: "recent".to_string(), visible: true },
                SectionConfig { id: "frequent".to_string(), visible: true },
                SectionConfig { id: "pinned".to_string(), visible: true },
            ],
            pinned_columns: 9,
            pinned_rows: 2,
            excluded_patterns: Vec::new(),
            debug_log: false,
            show_evac_button: false,
            hotkey_taps: 2,
            hotkey_interval_ms: 400,
            window_position: "center".to_string(),
            window_margin: 40,
            auto_backup: true,
            accent_color: "blue".to_string(),
            accent_custom: "#3b82f6".to_string(),
            background_image: String::new(),
            background_dim: 40,
            background_scale: 100,
            background_overflow: 0,
            window_opacity: 100,
            text_color: "auto".to_string(),
            text_custom: "#ffffff".to_string(),
        }
    }
}
