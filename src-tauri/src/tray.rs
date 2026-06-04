use crate::commands::evac;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItemBuilder::with_id("show", "ランチャーを表示").build(app)?;
    let settings = MenuItemBuilder::with_id("settings", "設定").build(app)?;
    let evac = MenuItemBuilder::with_id("evac", "DBをローカルに退避").build(app)?;
    let merge = MenuItemBuilder::with_id("merge", "Dropboxへマージ").build(app)?;
    let discard = MenuItemBuilder::with_id("discard", "退避を破棄").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "終了").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&settings)
        .separator()
        .item(&evac)
        .item(&merge)
        .item(&discard)
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
            "evac" => {
                let state = app.state::<crate::AppState>();
                let msg = match evac::do_evacuate(state.inner()) {
                    Ok(()) => "DBをローカルに退避しました".to_string(),
                    Err(e) => format!("退避に失敗: {}", e),
                };
                evac::emit_evac_changed(app);
                let _ = app.emit("evac-toast", msg);
            }
            "merge" => {
                let state = app.state::<crate::AppState>();
                let msg = match evac::do_merge(state.inner()) {
                    Ok(r) => {
                        if r.strategy == "overwrite" {
                            "Dropboxへ反映しました（上書き）".to_string()
                        } else {
                            format!(
                                "Dropboxへマージしました（追加 {} / 更新 {} / フォルダ {}）",
                                r.apps_added, r.apps_updated, r.folders_added
                            )
                        }
                    }
                    Err(e) => format!("マージに失敗: {}", e),
                };
                evac::emit_evac_changed(app);
                let _ = app.emit("evac-toast", msg);
            }
            "discard" => {
                let state = app.state::<crate::AppState>();
                let msg = match evac::do_discard(state.inner()) {
                    Ok(()) => "退避を破棄しました".to_string(),
                    Err(e) => format!("破棄に失敗: {}", e),
                };
                evac::emit_evac_changed(app);
                let _ = app.emit("evac-toast", msg);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
