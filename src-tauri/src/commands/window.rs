use crate::AppState;
use tauri::{Manager, State};

/// Calculate window size from grid layout.
///   width  = cols * 66 + 32  (grid: cols*64 + (cols-1)*2, padding: px-4*2=32, border: 2)
///   height = total_rows * 70 + section_count * 24 + 108
pub fn calc_window_size(cols: u32, total_rows: u32, section_count: u32) -> (u32, u32) {
    let w = cols * 66 + 32;
    let h = total_rows * 70 + section_count * 24 + 108;
    (w, h)
}

#[tauri::command]
pub async fn resize_window(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    cols: u32,
    total_rows: u32,
    section_count: u32,
) -> Result<(), String> {
    let (w, h) = calc_window_size(cols, total_rows, section_count);
    if let Ok(mut ls) = state.window_logical_size.lock() {
        *ls = (w, h);
    }
    if let Some(window) = app.get_webview_window("main") {
        let size = tauri::LogicalSize::new(w as f64, h as f64);
        window.set_size(tauri::Size::Logical(size)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_blur_hide(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut flag = state.blur_hide_enabled.lock().map_err(|e| e.to_string())?;
    *flag = enabled;
    Ok(())
}

#[tauri::command]
pub async fn get_exe_dir(state: State<'_, AppState>) -> Result<String, String> {
    let path = std::path::Path::new(&state.settings_path);
    let dir = path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_string_lossy()
        .to_string();
    Ok(dir)
}
