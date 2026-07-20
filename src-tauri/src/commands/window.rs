use crate::AppState;
use tauri::{Manager, State};

/// Calculate window size from grid layout.
///   width  = cols * 66 + 32  (grid: cols*64 + (cols-1)*2, padding: px-4*2=32, border: 2)
///   height = total_rows * 72 + section_count * 28 + 128
///
/// The per-row (72) covers the ~69px AppIcon button + the 2px gap-0.5 between
/// pinned grid rows. The per-section (28) covers the section label + mb-1 + mb-2
/// gap on inter-section spacing. The constant (128) covers the SearchBar,
/// borders, container padding (pt-2 + pb-5 = 28px), and a slack margin for
/// OEM-specific font metrics — leaving comfortable breathing room below
/// the last row.
pub fn calc_window_size(cols: u32, total_rows: u32, section_count: u32) -> (u32, u32) {
    let w = cols * 66 + 32;
    let h = total_rows * 72 + section_count * 28 + 128;
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
        let scale = window.scale_factor().unwrap_or(1.0);
        let w_phys = (w as f64 * scale).ceil() as u32;
        let h_phys = (h as f64 * scale).ceil() as u32;
        let size = tauri::PhysicalSize::new(w_phys, h_phys);
        window
            .set_size(tauri::Size::Physical(size))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Set the window height to a JS-measured CSS pixel value, keeping the
/// existing logical width. Used as a finishing correction on top of
/// `resize_window`'s formula so DPI rounding and font-metric variation
/// can't clip the bottom row.
/// Resize the launcher window to a CSS-pixel size, using the JS-reported
/// devicePixelRatio for the conversion to physical pixels. We *cannot* trust
/// `Window::scale_factor()` here — at certain Windows configurations
/// (e.g. system DPI 100% + per-monitor 125% custom scaling) it reports a
/// different value than the WebView's `window.devicePixelRatio`, which
/// leaves the layout cropped by exactly that ratio.
#[tauri::command]
pub async fn set_launcher_size(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    cols: u32,
    height: u32,
    dpr: f64,
    // `overflow` is the transparent bleed around the card, added to both sides.
    // The webview reports a height that already includes it, so only the width
    // is grown here.
    overflow: Option<u32>,
) -> Result<(), String> {
    let width_css = cols * 66 + 32 + overflow.unwrap_or(0) * 2;
    let dpr = if dpr.is_finite() && dpr > 0.0 { dpr } else { 1.0 };

    if let Ok(mut ls) = state.window_logical_size.lock() {
        *ls = (width_css, height);
    }
    if let Some(window) = app.get_webview_window("main") {
        let w_phys = (width_css as f64 * dpr).ceil() as u32;
        let h_phys = (height as f64 * dpr).ceil() as u32;
        let size = tauri::PhysicalSize::new(w_phys, h_phys);
        window
            .set_size(tauri::Size::Physical(size))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_blur_hide(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut flag = state.blur_hide_enabled.lock().map_err(|e| e.to_string())?;
    *flag = enabled;
    Ok(())
}

/// Re-apply window vibrancy when theme changes (light/dark).
/// `dark` = true → dark Mica/Acrylic, false → light.
///
/// `plain` disables the native backdrop entirely. Mica/Acrylic composite the
/// desktop behind the webview, which shows through a custom background image
/// and washes out reduced-opacity backgrounds — in those modes the CSS layer
/// has to be the only thing painting.
#[tauri::command]
pub async fn set_vibrancy_theme(
    app: tauri::AppHandle,
    dark: bool,
    plain: Option<bool>,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if let Some(window) = app.get_webview_window("main") {
            use window_vibrancy::{apply_acrylic, apply_mica, clear_acrylic, clear_mica};

            if plain.unwrap_or(false) {
                // Either call fails when that effect was never applied; the
                // goal is just to end up with no backdrop.
                let _ = clear_mica(&window);
                let _ = clear_acrylic(&window);
                return Ok(());
            }

            if apply_mica(&window, Some(dark)).is_ok() {
                return Ok(());
            }
            let color = if dark {
                (18u8, 18u8, 18u8, 200u8)
            } else {
                (240u8, 240u8, 240u8, 200u8)
            };
            apply_acrylic(&window, Some(color)).map_err(|e| e.to_string())?;
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (app, dark, plain);
    Ok(())
}

/// Copy a user-chosen background image next to the EXE and return the stored
/// path. Copying keeps the portable layout self-contained: the theme survives
/// the original file being moved, renamed or deleted, and travels with the
/// folder to another machine.
#[tauri::command]
pub async fn set_background_image(
    state: State<'_, AppState>,
    source: String,
) -> Result<String, String> {
    let src = std::path::Path::new(&source);
    if !src.is_file() {
        return Err("画像ファイルが見つかりません".to_string());
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    let base = std::path::Path::new(&state.settings_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    let theme_dir = base.join("theme");
    std::fs::create_dir_all(&theme_dir).map_err(|e| e.to_string())?;

    // Drop any previous image so stale extensions don't accumulate.
    if let Ok(rd) = std::fs::read_dir(&theme_dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.file_stem().and_then(|s| s.to_str()) == Some("background") {
                let _ = std::fs::remove_file(p);
            }
        }
    }

    let dest = theme_dir.join(format!("background.{}", ext));
    std::fs::copy(src, &dest).map_err(|e| format!("画像のコピーに失敗しました: {}", e))?;
    Ok(dest.to_string_lossy().to_string())
}

/// Read the stored background image as a base64 data URI for the webview.
/// The webview can't load arbitrary local paths, so the bytes are inlined.
#[tauri::command]
pub async fn get_background_image_data(path: String) -> Result<String, String> {
    use base64::Engine;

    let p = std::path::Path::new(&path);
    if !p.is_file() {
        return Err("画像ファイルが見つかりません".to_string());
    }
    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
    let mime = match p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => "image/png",
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
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
