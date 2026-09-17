//! Window management: the floating bubble, the chat window, settings and the
//! dictation overlay.
//!
//! The bubble is the resting state. Clicking it (or the global shortcut)
//! opens the chat window and hides the bubble; minimizing the chat returns to
//! the bubble.

use crate::settings::WindowGeometry;
use crate::state::AppState;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN: &str = "main";
pub const BUBBLE: &str = "bubble";
pub const SETTINGS: &str = "settings";
pub const OVERLAY: &str = "overlay";

const MARGIN: i32 = 16;
pub const COMPACT_SIZE: (f64, f64) = (380.0, 170.0);
/// Logical size of the bubble window (the visible circle is smaller, leaving room for its shadow).
pub const BUBBLE_SIZE: f64 = 84.0;

/// Timestamp (ms) until which Moved/Resized events are programmatic and must not be persisted.
static SUPPRESS_UNTIL: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn suppress_persistence() {
    SUPPRESS_UNTIL.store(now_ms() + 700, Ordering::Relaxed);
}

pub fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN)
}

/// Computes the top-left position for a preset inside the monitor work area.
pub fn preset_position(preset: &str, work_pos: (i32, i32), work_size: (u32, u32), win: (u32, u32)) -> (i32, i32) {
    let (wx, wy) = work_pos;
    let (ww, wh) = (work_size.0 as i32, work_size.1 as i32);
    let (w, h) = (win.0 as i32, win.1 as i32);
    match preset {
        "bottom-left" => (wx + MARGIN, wy + wh - h - MARGIN),
        "center" => (wx + (ww - w) / 2, wy + (wh - h) / 2),
        _ => (wx + ww - w - MARGIN, wy + wh - h - MARGIN),
    }
}

/// Clamps a window rectangle into the work area so it is always fully visible.
pub fn clamp_into(work_pos: (i32, i32), work_size: (u32, u32), pos: (i32, i32), win: (u32, u32)) -> (i32, i32) {
    let max_x = work_pos.0 + work_size.0 as i32 - win.0 as i32;
    let max_y = work_pos.1 + work_size.1 as i32 - win.1 as i32;
    (pos.0.clamp(work_pos.0, max_x.max(work_pos.0)), pos.1.clamp(work_pos.1, max_y.max(work_pos.1)))
}

fn on_any_monitor(window: &WebviewWindow, x: i32, y: i32) -> bool {
    window
        .available_monitors()
        .map(|ms| {
            ms.iter().any(|m| {
                let p = m.position();
                let s = m.size();
                x >= p.x - 50 && y >= p.y - 50 && x < p.x + s.width as i32 - 50 && y < p.y + s.height as i32 - 50
            })
        })
        .unwrap_or(false)
}

pub fn apply_position(window: &WebviewWindow, preset: &str, custom: &WindowGeometry) {
    suppress_persistence();
    let Ok(size) = window.outer_size() else { return };
    if preset == "custom" && on_any_monitor(window, custom.x, custom.y) {
        let _ = window.set_position(PhysicalPosition::new(custom.x, custom.y));
        return;
    }
    let monitor = window.current_monitor().ok().flatten().or_else(|| window.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let area = m.work_area();
        let (x, y) = preset_position(preset, (area.position.x, area.position.y), (area.size.width, area.size.height), (size.width, size.height));
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

/// Restores chat window size/always-on-top from settings and places the bubble.
pub fn restore(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings.get().general;
    if let Some(win) = main_window(app) {
        suppress_persistence();
        if s.compact {
            let _ = win.set_size(LogicalSize::new(COMPACT_SIZE.0, COMPACT_SIZE.1));
        } else if s.window.width > 0 && s.window.height > 0 {
            let _ = win.set_size(PhysicalSize::new(s.window.width, s.window.height));
        }
        let _ = win.set_always_on_top(s.always_on_top);
        apply_position(&win, &s.window_position, &s.window);
    }
    let _ = create_bubble(app);
}

pub fn create_bubble(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(BUBBLE) {
        return Ok(w);
    }
    let w = WebviewWindowBuilder::new(app, BUBBLE, WebviewUrl::App("index.html#/bubble".into()))
        .title("Local Assistant")
        .inner_size(BUBBLE_SIZE, BUBBLE_SIZE)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()?;
    place_bubble(app, &w);
    let h = app.clone();
    w.on_window_event(move |e| match e {
        tauri::WindowEvent::Moved(pos) => {
            if now_ms() < SUPPRESS_UNTIL.load(Ordering::Relaxed) {
                return;
            }
            let (x, y) = (pos.x, pos.y);
            let _ = h.state::<AppState>().settings.update(|s| {
                s.general.bubble_x = Some(x);
                s.general.bubble_y = Some(y);
            });
        }
        tauri::WindowEvent::CloseRequested { api, .. } => api.prevent_close(),
        _ => {}
    });
    Ok(w)
}

fn place_bubble(app: &AppHandle, w: &WebviewWindow) {
    suppress_persistence();
    let s = app.state::<AppState>().settings.get().general;
    let size = w.outer_size().unwrap_or(PhysicalSize::new(84, 84));
    if let (Some(x), Some(y)) = (s.bubble_x, s.bubble_y) {
        if on_any_monitor(w, x, y) {
            let _ = w.set_position(PhysicalPosition::new(x, y));
            return;
        }
    }
    if let Some(m) = w.primary_monitor().ok().flatten() {
        let area = m.work_area();
        let (x, y) = preset_position("bottom-right", (area.position.x, area.position.y), (area.size.width, area.size.height), (size.width, size.height));
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
}

pub fn show_bubble(app: &AppHandle) {
    if let Ok(b) = create_bubble(app) {
        let _ = b.show();
    }
}

pub fn hide_bubble(app: &AppHandle) {
    if let Some(b) = app.get_webview_window(BUBBLE) {
        let _ = b.hide();
    }
}

/// Opens the chat window next to where the bubble is (unless the user placed the chat window).
pub fn show_main(app: &AppHandle, focus_input: bool) {
    let Some(win) = main_window(app) else { return };
    let s = app.state::<AppState>().settings.get().general;
    if !win.is_visible().unwrap_or(false) && s.window_position != "custom" {
        if let (Some(bubble), Ok(size)) = (app.get_webview_window(BUBBLE), win.outer_size()) {
            if bubble.is_visible().unwrap_or(false) {
                if let (Ok(bp), Ok(bs), Ok(Some(m))) = (bubble.outer_position(), bubble.outer_size(), bubble.current_monitor()) {
                    // Anchor the chat's bottom-right corner to the bubble's bottom-right corner.
                    let area = m.work_area();
                    let desired = (bp.x + bs.width as i32 - size.width as i32, bp.y + bs.height as i32 - size.height as i32);
                    let (x, y) = clamp_into((area.position.x, area.position.y), (area.size.width, area.size.height), desired, (size.width, size.height));
                    suppress_persistence();
                    let _ = win.set_position(PhysicalPosition::new(x, y));
                }
            }
        }
    }
    if win.is_minimized().unwrap_or(false) {
        let _ = win.unminimize();
    }
    let _ = win.show();
    let _ = win.set_focus();
    hide_bubble(app);
    if focus_input {
        let _ = app.emit_to(MAIN, "app://focus-input", ());
    }
}

/// Hides every window: the app keeps running in the system tray.
pub fn hide_to_tray(app: &AppHandle) {
    for label in [MAIN, BUBBLE, OVERLAY] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.hide();
        }
    }
}

/// Hides the chat window and returns to the floating bubble.
pub fn minimize_to_bubble(app: &AppHandle) {
    if let Some(w) = main_window(app) {
        let _ = w.hide();
    }
    show_bubble(app);
}

pub fn toggle_main(app: &AppHandle) {
    let Some(win) = main_window(app) else { return };
    let visible = win.is_visible().unwrap_or(false);
    let focused = win.is_focused().unwrap_or(false);
    if visible && focused {
        minimize_to_bubble(app);
    } else {
        show_main(app, true);
    }
}

/// Persists user-driven moves/resizes of the chat window.
pub fn on_main_window_event(app: &AppHandle, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // Closing (X or Alt+F4) hides to the tray; quitting happens from the tray menu.
            api.prevent_close();
            hide_to_tray(app);
        }
        tauri::WindowEvent::Moved(pos) => {
            if now_ms() < SUPPRESS_UNTIL.load(Ordering::Relaxed) {
                return;
            }
            let state = app.state::<AppState>();
            let (x, y) = (pos.x, pos.y);
            let _ = state.settings.update(|s| {
                s.general.window.x = x;
                s.general.window.y = y;
                s.general.window_position = "custom".into();
            });
        }
        tauri::WindowEvent::Resized(size) => {
            if now_ms() < SUPPRESS_UNTIL.load(Ordering::Relaxed) || size.width == 0 || size.height == 0 {
                return;
            }
            let state = app.state::<AppState>();
            if state.settings.get().general.compact {
                return;
            }
            let (w, h) = (size.width, size.height);
            let _ = state.settings.update(|s| {
                s.general.window.width = w;
                s.general.window.height = h;
            });
        }
        _ => {}
    }
}

pub fn set_compact(app: &AppHandle, compact: bool) {
    let state = app.state::<AppState>();
    let Ok(s) = state.settings.update(|s| s.general.compact = compact) else { return };
    let Some(win) = main_window(app) else { return };
    suppress_persistence();
    if compact {
        let _ = win.set_size(LogicalSize::new(COMPACT_SIZE.0, COMPACT_SIZE.1));
    } else {
        let g = &s.general.window;
        let _ = win.set_size(PhysicalSize::new(g.width, g.height));
    }
    if s.general.window_position != "custom" {
        apply_position(&win, &s.general.window_position, &s.general.window);
    }
}

pub fn open_settings(app: &AppHandle, section: Option<&str>) -> tauri::Result<()> {
    let route = format!("index.html#/settings/{}", section.unwrap_or("general"));
    if let Some(w) = app.get_webview_window(SETTINGS) {
        let _ = app.emit_to(SETTINGS, "app://navigate", format!("/settings/{}", section.unwrap_or("general")));
        let _ = w.unminimize();
        w.show()?;
        w.set_focus()?;
        return Ok(());
    }
    let w = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App(route.into()))
        .title("Local Assistant — Settings")
        .inner_size(1000.0, 720.0)
        .min_inner_size(720.0, 520.0)
        .center()
        .visible(true)
        .build()?;
    w.set_focus()?;
    Ok(())
}

pub fn create_overlay(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(OVERLAY) {
        return Ok(w);
    }
    WebviewWindowBuilder::new(app, OVERLAY, WebviewUrl::App("index.html#/overlay".into()))
        .title("Dictation")
        .inner_size(440.0, 128.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()
}

/// Shows the dictation overlay near the bottom center without taking focus.
pub fn show_overlay(app: &AppHandle) {
    let Ok(w) = create_overlay(app) else { return };
    if let Ok(Some(m)) = w.primary_monitor() {
        let area = m.work_area();
        let size = w.outer_size().unwrap_or(PhysicalSize::new(440, 128));
        let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
        let y = area.position.y + area.size.height as i32 - size.height as i32 - 40;
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    let _ = w.show();
}

pub fn hide_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(OVERLAY) {
        let _ = w.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::{clamp_into, preset_position};

    #[test]
    fn presets() {
        let work = ((0, 0), (1920, 1040));
        let win = (400, 600);
        assert_eq!(preset_position("bottom-right", work.0, work.1, win), (1504, 424));
        assert_eq!(preset_position("bottom-left", work.0, work.1, win), (16, 424));
        assert_eq!(preset_position("center", work.0, work.1, win), (760, 220));
        // Secondary monitor to the left with an offset origin
        assert_eq!(preset_position("bottom-right", (-1280, 0), (1280, 984), win), (-416, 368));
    }

    #[test]
    fn chat_anchored_to_bubble_stays_on_screen() {
        // Bubble dragged to the top-left corner: chat would go off-screen, so it is clamped.
        assert_eq!(clamp_into((0, 0), (1920, 1040), (-300, -500), (400, 600)), (0, 0));
        assert_eq!(clamp_into((0, 0), (1920, 1040), (1600, 500), (400, 600)), (1520, 440));
        assert_eq!(clamp_into((0, 0), (1920, 1040), (100, 100), (400, 600)), (100, 100));
    }
}
