use super::ScannedApp;
use std::path::Path;
use walkdir::WalkDir;

pub fn scan(dir: &Path) -> Vec<ScannedApp> {
    let mut apps = Vec::new();

    for entry in WalkDir::new(dir)
        .max_depth(3)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Look for .app bundles
        if path.extension().and_then(|e| e.to_str()) == Some("app") && path.is_dir() {
            let name = extract_app_name_from_bundle(path).unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string()
            });

            let exe_path = path.to_string_lossy().to_string();
            apps.push(ScannedApp { name, exe_path });
        }
    }

    apps
}

fn extract_app_name_from_bundle(app_path: &Path) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");
    let plist_data = plist::Value::from_file(&plist_path).ok()?;
    let dict = plist_data.as_dictionary()?;

    if let Some(name) = dict.get("CFBundleDisplayName").and_then(|v| v.as_string()) {
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    if let Some(name) = dict.get("CFBundleName").and_then(|v| v.as_string()) {
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    None
}
