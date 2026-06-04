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
        }
    }
}
