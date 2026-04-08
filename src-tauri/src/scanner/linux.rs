use super::ScannedApp;
use std::path::Path;
use walkdir::WalkDir;

pub fn scan(dir: &Path) -> Vec<ScannedApp> {
    let mut apps = Vec::new();

    for entry in WalkDir::new(dir)
        .max_depth(5)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Check if file is executable
        use std::os::unix::fs::PermissionsExt;
        let is_executable = path
            .metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);

        if !is_executable {
            continue;
        }

        // Skip scripts and common non-app files
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.starts_with('.') || file_name.contains(".so") {
            continue;
        }

        let name = file_name.to_string();
        let exe_path = path.to_string_lossy().to_string();
        apps.push(ScannedApp { name, exe_path });
    }

    apps
}
