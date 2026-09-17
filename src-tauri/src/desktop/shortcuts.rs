//! Global keyboard shortcuts: open/focus, push-to-talk and dictation.

use super::window;
use crate::services::stt::session::ListenMode;
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Action {
    Toggle,
    PushToTalk,
    Dictation,
}

fn configured(app: &AppHandle) -> Vec<(Action, String)> {
    let s = app.state::<AppState>().settings.get();
    let mut v = vec![(Action::Toggle, s.general.global_shortcut.clone())];
    if s.stt.push_to_talk {
        v.push((Action::PushToTalk, s.general.push_to_talk_shortcut.clone()));
    }
    if s.dictation.enabled {
        v.push((Action::Dictation, s.dictation.shortcut.clone()));
    }
    v.into_iter().filter(|(_, k)| !k.trim().is_empty()).collect()
}

/// Temporarily releases all global shortcuts so the key combination reaches
/// the settings window while the user is recording a new one.
pub fn unregister_all(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
}

/// (Re-)registers all shortcuts; failures (e.g. taken by another app) are
/// recorded for the settings UI instead of aborting.
pub fn register_all(app: &AppHandle) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut errors = Vec::new();
    let mut seen: Vec<u32> = Vec::new();
    for (action, keys) in configured(app) {
        match keys.parse::<Shortcut>() {
            Ok(sc) => {
                if seen.contains(&sc.id()) {
                    errors.push(format!("{keys}: used by more than one action"));
                    continue;
                }
                seen.push(sc.id());
                if let Err(e) = gs.register(sc) {
                    tracing::warn!(?action, keys, error = %e, "shortcut registration failed");
                    errors.push(format!("{keys}: {e}"));
                }
            }
            Err(e) => errors.push(format!("{keys}: {e}")),
        }
    }
    *app.state::<AppState>().shortcut_errors.lock().unwrap_or_else(|p| p.into_inner()) = errors;
}

pub fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    let Some(action) = configured(app)
        .into_iter()
        .find(|(_, keys)| keys.parse::<Shortcut>().map(|s| s.id() == shortcut.id()).unwrap_or(false))
        .map(|(a, _)| a)
    else {
        return;
    };
    let state = app.state::<AppState>();
    let pressed = event.state() == ShortcutState::Pressed;
    match action {
        Action::Toggle => {
            if pressed {
                window::toggle_main(app);
            }
        }
        Action::PushToTalk => {
            if pressed {
                if state.voice.active_mode() != Some(ListenMode::PushToTalk) {
                    window::show_main(app, false);
                    state.tts.stop_all();
                    if let Err(e) = state.voice.start(ListenMode::PushToTalk) {
                        let _ = app.emit("voice://error", &e);
                    }
                }
            } else if state.voice.active_mode() == Some(ListenMode::PushToTalk) {
                state.voice.stop(false);
            }
        }
        Action::Dictation => {
            let hold = state.settings.get().dictation.mode == "hold";
            let active = state.voice.active_mode() == Some(ListenMode::Dictation);
            if pressed && !active {
                if state.dictation_busy.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                match state.voice.start(ListenMode::Dictation) {
                    Ok(()) => window::show_overlay(app),
                    Err(e) => {
                        window::show_overlay(app);
                        let _ = app.emit("dictation://state", serde_json::json!({"state": "error", "error": e}));
                    }
                }
            } else if active && ((hold && !pressed) || (!hold && pressed)) {
                state.voice.stop(false);
            }
        }
    }
}
