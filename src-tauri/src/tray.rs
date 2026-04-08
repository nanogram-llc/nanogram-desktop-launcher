use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItemBuilder::with_id("show", "ランチャーを表示").build(app)?;
    let settings = MenuItemBuilder::with_id("settings", "設定").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "終了").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&settings)
        .separator()
        .item(&quit)
        .build()?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().expect("no app icon"))
        .tooltip("nanogram desktop LauncheR")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(e) = window.show() {
                        eprintln!("[tray:show] window.show() failed: {}", e);
                    }
                    if let Err(e) = window.set_focus() {
                        eprintln!("[tray:show] window.set_focus() failed: {}", e);
                    }
                } else {
                    eprintln!("[tray:show] window 'main' not found!");
                }
            }
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(e) = window.show() {
                        eprintln!("[tray:settings] window.show() failed: {}", e);
                    }
                    let _ = window.set_focus();
                    let _ = window.emit("navigate", "settings");
                } else {
                    eprintln!("[tray:settings] window 'main' not found!");
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
