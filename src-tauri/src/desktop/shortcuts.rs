//! Global keyboard shortcuts: open/focus, push-to-talk and dictation.

use super::window;
use crate::services::stt::session::ListenMode;
use crate::state::AppState;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
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

fn is_modifier_only_shortcut(keys: &str) -> bool {
    let tokens: Vec<&str> = keys
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        return false;
    }
    tokens.iter().all(|token| match token.to_ascii_uppercase().as_str() {
        "ALT" | "OPTION" => true,
        "CONTROL" | "CTRL" | "COMMANDORCONTROL" | "COMMANDORCTRL" | "CMDORCTRL" | "CMDORCONTROL" => true,
        "SHIFT" => true,
        "SUPER" | "META" | "COMMAND" | "CMD" | "WIN" => true,
        _ => false,
    })
}

/// Single modifiers (e.g. "Alt") fire on every normal press of that key
/// (Alt+Tab, Alt+F4, menu focus). Require 2+ modifiers for modifier-only.
fn modifier_only_count(keys: &str) -> usize {
    keys.split('+').map(str::trim).filter(|t| !t.is_empty()).count()
}

fn is_supported_modifier_only(keys: &str) -> bool {
    is_modifier_only_shortcut(keys) && modifier_only_count(keys) >= 2
}

#[cfg(windows)]
fn windows_modifier_pressed(token: &str) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
        VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
    };
    let pressed = |vk: i32| unsafe { (GetAsyncKeyState(vk) as i16) < 0 };
    match token.to_ascii_uppercase().as_str() {
        "ALT" | "OPTION" => pressed(VK_MENU.0 as i32) || pressed(VK_LMENU.0 as i32) || pressed(VK_RMENU.0 as i32),
        "CONTROL" | "CTRL" | "COMMANDORCONTROL" | "COMMANDORCTRL" | "CMDORCTRL" | "CMDORCONTROL" => {
            pressed(VK_CONTROL.0 as i32) || pressed(VK_LCONTROL.0 as i32) || pressed(VK_RCONTROL.0 as i32)
        }
        "SHIFT" => pressed(VK_SHIFT.0 as i32) || pressed(VK_LSHIFT.0 as i32) || pressed(VK_RSHIFT.0 as i32),
        "SUPER" | "META" | "COMMAND" | "CMD" | "WIN" => pressed(VK_LWIN.0 as i32) || pressed(VK_RWIN.0 as i32),
        _ => false,
    }
}

#[cfg(windows)]
fn windows_modifier_shortcut_pressed(keys: &str) -> bool {
    keys.split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .all(windows_modifier_pressed)
}

fn dispatch_shortcut_action(app: &AppHandle, action: Action, pressed: bool) {
    let state = app.state::<AppState>();
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

#[cfg(windows)]
fn trigger_modifier_watch(app: &AppHandle) {
    use std::sync::{Mutex, OnceLock};
    static COMBOS: OnceLock<std::sync::Arc<Mutex<Vec<(Action, String)>>>> = OnceLock::new();
    static WATCHER: OnceLock<Mutex<Option<std::thread::JoinHandle<()>>>> = OnceLock::new();
    let combos_shared = COMBOS.get_or_init(|| std::sync::Arc::new(Mutex::new(Vec::new()))).clone();

    // Refresh the watched combos on every register_all (user may have changed them).
    *combos_shared.lock().unwrap_or_else(|p| p.into_inner()) = configured(app)
        .into_iter()
        .filter(|(_, keys)| is_supported_modifier_only(keys))
        .collect();

    let watcher = WATCHER.get_or_init(|| Mutex::new(None));
    let mut current = watcher.lock().unwrap();
    if current.is_some() {
        return;
    }

    let app = app.clone();
    *current = Some(thread::spawn(move || {
        let mut last = HashMap::new();
        loop {
            let snapshot: Vec<(Action, String)> = combos_shared.lock().map(|g| g.clone()).unwrap_or_default();
            for (action, keys) in &snapshot {
                let pressed = windows_modifier_shortcut_pressed(keys);
                let prev = *last.get(action).unwrap_or(&false);
                if pressed != prev {
                    dispatch_shortcut_action(&app, *action, pressed);
                }
                last.insert(*action, pressed);
            }
            // Drop stale actions (shortcut changed away) so a stuck "pressed"
            // state can't leak into the next combo with the same action.
            last.retain(|a, _| snapshot.iter().any(|(sa, _)| sa == a));
            thread::sleep(Duration::from_millis(25));
        }
    }));
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
        if is_modifier_only_shortcut(&keys) {
            if !is_supported_modifier_only(&keys) {
                errors.push(format!("{keys}: single-modifier shortcuts are disabled (use 2+ modifiers or modifier+key)"));
                continue;
            }
            #[cfg(not(windows))]
            errors.push(format!("{keys}: modifier-only shortcuts are Windows-only"));
            continue;
        }
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
    #[cfg(windows)]
    {
        // Always refresh watcher combos (clears stale single-Alt, etc).
        trigger_modifier_watch(app);
    }
    *app.state::<AppState>().shortcut_errors.lock().unwrap_or_else(|p| p.into_inner()) = errors;
}

pub fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    let Some(action) = configured(app)
        .into_iter()
        .find(|(_, keys)| !is_modifier_only_shortcut(keys) && keys.parse::<Shortcut>().map(|s| s.id() == shortcut.id()).unwrap_or(false))
        .map(|(a, _)| a)
    else {
        return;
    };
    dispatch_shortcut_action(app, action, event.state() == ShortcutState::Pressed);
}

#[cfg(test)]
mod tests {
    use super::{is_modifier_only_shortcut, is_supported_modifier_only};

    #[test]
    fn single_modifier_not_supported() {
        assert!(is_modifier_only_shortcut("Alt"));
        assert!(!is_supported_modifier_only("Alt"));
        assert!(!is_supported_modifier_only("Shift"));
        assert!(is_supported_modifier_only("CommandOrControl+Alt"));
        assert!(is_supported_modifier_only("Alt+Shift"));
        assert!(!is_supported_modifier_only("CommandOrControl+Space"));
    }
}
