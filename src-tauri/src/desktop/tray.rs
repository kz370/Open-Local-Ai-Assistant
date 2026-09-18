use super::window;
use crate::state::AppState;
use std::sync::OnceLock;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

/// The "Live Captions" checkbox, kept in step with the caption session.
static CAPTIONS_ITEM: OnceLock<CheckMenuItem<Wry>> = OnceLock::new();

pub fn set_captions_checked(on: bool) {
    if let Some(item) = CAPTIONS_ITEM.get() {
        let _ = item.set_checked(on);
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let title = MenuItem::with_id(app, "title", "Local Assistant", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let new_conv = MenuItem::with_id(app, "new", "New Conversation", true, None::<&str>)?;
    let voice = MenuItem::with_id(app, "voice", "Voice Mode", true, None::<&str>)?;
    let captions = CheckMenuItem::with_id(app, "captions", "Live Captions", true, false, None::<&str>)?;
    let _ = CAPTIONS_ITEM.set(captions.clone());
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
            &captions,
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
            // The checkbox flips itself on click; the caption session re-syncs it.
            "captions" => window::toggle_captions(app),
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
    state.captions.stop();
    for label in [window::MAIN, window::BUBBLE, window::OVERLAY, window::CAPTIONS, window::SETTINGS] {
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
