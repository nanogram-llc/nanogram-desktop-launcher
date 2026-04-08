#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod windows;

use std::path::Path;

#[derive(Debug)]
pub struct ScannedApp {
    pub name: String,
    pub exe_path: String,
}

pub fn scan_directory(dir: &Path) -> Vec<ScannedApp> {
    #[cfg(target_os = "windows")]
    return windows::scan(dir);

    #[cfg(target_os = "macos")]
    return macos::scan(dir);

    #[cfg(target_os = "linux")]
    return linux::scan(dir);
}

/// Extract a display name from an exe file (PE metadata).
pub fn extract_name_from_exe(path: &Path) -> Option<String> {
    #[cfg(target_os = "windows")]
    return windows::extract_app_name_pub(path);

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        None
    }
}

/// Resolve a single .lnk file to a ScannedApp.
pub fn resolve_single_lnk(path: &Path) -> Option<ScannedApp> {
    #[cfg(target_os = "windows")]
    return windows::resolve_lnk_pub(path);

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        None
    }
}
