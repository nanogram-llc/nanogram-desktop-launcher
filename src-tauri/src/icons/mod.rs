#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use std::path::Path;

/// Extract icon from an executable and save as PNG to the cache directory.
/// Returns the path to the cached PNG file.
pub fn extract_and_cache_icon(exe_path: &str, cache_dir: &Path, app_id: &str) -> Option<String> {
    let output_path = cache_dir.join(format!("{}.png", app_id));

    #[cfg(target_os = "windows")]
    let result = windows::extract_icon(exe_path, &output_path);

    #[cfg(target_os = "macos")]
    let result = macos::extract_icon(exe_path, &output_path);

    #[cfg(target_os = "linux")]
    let result = linux::extract_icon(exe_path, &output_path);

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let result = false;

    if result {
        Some(output_path.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Extract icon for any file or folder (not just executables) and cache as PNG.
pub fn extract_file_icon_and_cache(path: &str, cache_dir: &Path, app_id: &str) -> Option<String> {
    let output_path = cache_dir.join(format!("{}.png", app_id));

    #[cfg(target_os = "windows")]
    let result = windows::extract_file_icon(path, &output_path);

    // Fallback for non-Windows: try the exe extractor
    #[cfg(not(target_os = "windows"))]
    let result = {
        let _ = &output_path;
        return extract_and_cache_icon(path, cache_dir, app_id);
    };

    if result {
        Some(output_path.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Load a cached icon file and return it as a base64-encoded data URI.
pub fn load_icon_as_base64(icon_path: &str) -> Option<String> {
    use base64::Engine;
    let data = std::fs::read(icon_path).ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Some(format!("data:image/png;base64,{}", b64))
}
