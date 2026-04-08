#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Prevent duplicate instances using a named mutex (Windows only)
    #[cfg(target_os = "windows")]
    {
        // Use raw FFI to avoid the `windows` crate clearing GetLastError
        #[link(name = "kernel32")]
        extern "system" {
            fn CreateMutexW(
                lp_mutex_attributes: *const std::ffi::c_void,
                b_initial_owner: i32,
                lp_name: *const u16,
            ) -> *mut std::ffi::c_void;
            fn GetLastError() -> u32;
        }

        const ERROR_ALREADY_EXISTS: u32 = 183;
        // "Global\NanogramLauncher_SI" — Global namespace works across sessions
        const MUTEX_NAME: &[u16] = &[
            b'G' as u16, b'l' as u16, b'o' as u16, b'b' as u16, b'a' as u16,
            b'l' as u16, b'\\' as u16,
            b'N' as u16, b'a' as u16, b'n' as u16, b'o' as u16, b'g' as u16,
            b'r' as u16, b'a' as u16, b'm' as u16, b'L' as u16, b'a' as u16,
            b'u' as u16, b'n' as u16, b'c' as u16, b'h' as u16, b'e' as u16,
            b'r' as u16, b'_' as u16, b'S' as u16, b'I' as u16, 0,
        ];

        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, MUTEX_NAME.as_ptr()) };
        let err = unsafe { GetLastError() };

        // Diagnostic: write result to a log file next to the exe
        let log_msg = format!(
            "[{}] pid={} handle={:?} err={} already_exists={}\n",
            chrono_now(),
            std::process::id(),
            handle,
            err,
            err == ERROR_ALREADY_EXISTS
        );
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let log_path = dir.join("single_instance_debug.log");
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                {
                    let _ = f.write_all(log_msg.as_bytes());
                }
            }
        }

        if handle.is_null() || err == ERROR_ALREADY_EXISTS {
            // Another instance is already running — try to show its window, then exit
            activate_existing_instance();
            return;
        }

        // Handle stays open (leaked) until process exit — keeps the mutex alive.
    }

    launcher_lib::run()
}

#[cfg(target_os = "windows")]
fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", secs)
}

/// Second instance detected — just exit. Do NOT force-show the first
/// instance's window: it may still be initializing (hotkey thread, blur_hide
/// handler, vibrancy), and calling ShowWindow mid-init leaves it in a broken
/// state. Let the user summon it via the hotkey or tray.
#[cfg(target_os = "windows")]
fn activate_existing_instance() {
    // Intentionally no-op. Exit silently.
}
