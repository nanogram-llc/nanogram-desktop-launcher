use super::ScannedApp;
use std::path::Path;
use walkdir::WalkDir;

pub fn scan(dir: &Path) -> Vec<ScannedApp> {
    let mut apps = Vec::new();
    let mut seen_exe_paths = std::collections::HashSet::new();

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

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "lnk" => {
                if let Some(app) = resolve_lnk(path) {
                    if seen_exe_paths.insert(app.exe_path.to_lowercase()) {
                        apps.push(app);
                    }
                }
            }
            "exe" => {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if should_skip(file_name) {
                    continue;
                }

                let exe_path = path.to_string_lossy().to_string();
                if !seen_exe_paths.insert(exe_path.to_lowercase()) {
                    continue;
                }

                let name = extract_app_name(path).unwrap_or_else(|| {
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(file_name);
                    let parent = path
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    if parent.is_empty() || parent.eq_ignore_ascii_case(stem) {
                        stem.to_string()
                    } else {
                        format!("{}\\{}", parent, stem)
                    }
                });

                apps.push(ScannedApp { name, exe_path });
            }
            _ => {}
        }
    }

    apps
}

/// Resolve a .lnk shortcut file: extract the target exe path and use the shortcut name as app name.
fn resolve_lnk(lnk_path: &Path) -> Option<ScannedApp> {
    let target = resolve_lnk_target(lnk_path)?;
    let target_path = Path::new(&target);

    // Only accept .exe targets
    let ext = target_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "exe" {
        return None;
    }

    // Target must exist
    if !target_path.is_file() {
        return None;
    }

    let file_name = target_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if should_skip(file_name) {
        return None;
    }

    // Use the .lnk file stem as the display name (e.g., "Google Chrome.lnk" -> "Google Chrome")
    let name = lnk_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name)
        .to_string();

    Some(ScannedApp {
        name,
        exe_path: target,
    })
}

/// Use Windows COM IShellLink to resolve a .lnk file's target path.
#[cfg(target_os = "windows")]
fn resolve_lnk_target(lnk_path: &Path) -> Option<String> {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::IPersistFile;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let shell_link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;

        let persist_file: IPersistFile = shell_link.cast().ok()?;

        let wide_path: Vec<u16> = lnk_path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        use windows::Win32::System::Com::STGM;
        persist_file
            .Load(PCWSTR(wide_path.as_ptr()), STGM(0))
            .ok()?;

        let mut target_buf = [0u16; 260];
        shell_link
            .GetPath(&mut target_buf, std::ptr::null_mut(), 0)
            .ok()?;

        CoUninitialize();

        let len = target_buf
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(target_buf.len());
        let target = String::from_utf16_lossy(&target_buf[..len]);

        if target.is_empty() {
            None
        } else {
            Some(target)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn resolve_lnk_target(_lnk_path: &Path) -> Option<String> {
    None
}

/// Public wrapper for extract_app_name (used by register_files).
pub fn extract_app_name_pub(path: &Path) -> Option<String> {
    extract_app_name(path)
}

/// Public wrapper for resolve_lnk (used by register_files).
pub fn resolve_lnk_pub(lnk_path: &Path) -> Option<ScannedApp> {
    resolve_lnk(lnk_path)
}

fn should_skip(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    let skip_patterns = [
        "unins",
        "setup",
        "install",
        "update",
        "crash",
        "helper",
        "service",
        "daemon",
        "worker",
        "runtime",
        "webview",
        "createdump",
        "redist",
        "vcredist",
    ];
    skip_patterns.iter().any(|p| lower.contains(p))
}

fn extract_app_name(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let pe = pelite::PeFile::from_bytes(&data).ok()?;

    let resources = pe.resources().ok()?;
    let version_info = resources.version_info().ok()?;
    let lang = version_info.translation().first().copied().unwrap_or(
        pelite::resources::version_info::Language {
            lang_id: 0x0409,
            charset_id: 0x04B0,
        },
    );

    let mut file_description: Option<String> = None;
    let mut product_name: Option<String> = None;

    version_info.strings(lang, |key: &str, value: &str| {
        if key == "FileDescription" && !value.trim().is_empty() && file_description.is_none() {
            file_description = Some(value.trim().to_string());
        }
        if key == "ProductName" && !value.trim().is_empty() && product_name.is_none() {
            product_name = Some(value.trim().to_string());
        }
    });

    file_description.or(product_name)
}
