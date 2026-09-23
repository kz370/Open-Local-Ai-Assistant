use super::window;
use crate::state::AppState;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let title = MenuItem::with_id(app, "title", "Local Assistant", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let new_conv = MenuItem::with_id(app, "new", "New Conversation", true, None::<&str>)?;
    let voice = MenuItem::with_id(app, "voice", "Voice Mode", true, None::<&str>)?;
    let dictation = MenuItem::with_id(app, "dictation", "Dictation", true, None::<&str>)?;
    let dictation_history = MenuItem::with_id(app, "dictation-history", "Dictation History", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let diagnostics = MenuItem::with_id(app, "diagnostics", "Diagnostics", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &title,
            &open,
            &new_conv,
            &voice,
            &dictation,
            &dictation_history,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &diagnostics,
            &PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Local Assistant")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => window::show_main(app, true),
            "new" => {
                window::show_main(app, true);
                let _ = app.emit_to(window::MAIN, "app://new-conversation", ());
            }
            "voice" => {
                window::show_main(app, false);
                let _ = app.emit_to(window::MAIN, "app://toggle-hands-free", ());
            }
            "dictation" => super::shortcuts::toggle_dictation(app),
            "dictation-history" => {
                let _ = window::open_settings(app, Some("dictation"));
            }
            "settings" => {
                let _ = window::open_settings(app, None);
            }
            "diagnostics" => {
                let _ = window::open_settings(app, Some("diagnostics"));
            }
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                window::toggle_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.chat.stop_all();
    state.voice.stop(true);
    state.tts.stop_all();
    state.silma.stop();
    for label in [window::MAIN, window::BUBBLE, window::OVERLAY, window::SETTINGS] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.hide();
        }
    }
    let mcp = state.mcp.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(3), mcp.shutdown()).await;
        app.exit(0);
    });
}
