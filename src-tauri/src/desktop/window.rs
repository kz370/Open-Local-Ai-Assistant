//! Window management: the floating bubble, the chat window, settings and the
//! dictation overlay.
//!
//! The bubble is the resting state. Clicking it (or the global shortcut)
//! opens the chat window and hides the bubble; minimizing the chat returns to
//! the bubble.

use crate::settings::WindowGeometry;
use crate::services::stt::session::ListenMode;
use crate::state::AppState;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const MAIN: &str = "main";
pub const BUBBLE: &str = "bubble";
pub const SETTINGS: &str = "settings";
/// Default settings window height: fits the full sidebar without scrolling.
const SETTINGS_H: f64 = 820.0;
pub const OVERLAY: &str = "overlay";

const MARGIN: i32 = 16;
pub const COMPACT_SIZE: (f64, f64) = (380.0, 170.0);
/// Logical size of the bubble window (the visible circle is smaller, leaving room for its shadow).
pub const BUBBLE_SIZE: f64 = 84.0;
/// Chat window bounds outside compact mode: narrow/short enough and the
/// header, messages and composer start overlapping instead of stacking.
const CHAT_MIN_WIDTH: f64 = 480.0;
const CHAT_MAX_WIDTH: f64 = 760.0;
const CHAT_MIN_HEIGHT: f64 = 480.0;

/// Locks the window to an exact size in compact mode, or applies the normal
/// chat size bounds otherwise (drag-resize can never go smaller/larger than
/// the layout can actually render).
fn apply_size_bounds(win: &WebviewWindow, compact: bool) {
    if compact {
        let _ = win.set_min_size(Some(LogicalSize::new(COMPACT_SIZE.0, COMPACT_SIZE.1)));
        let _ = win.set_max_size(Some(LogicalSize::new(COMPACT_SIZE.0, COMPACT_SIZE.1)));
    } else {
        let _ = win.set_min_size(Some(LogicalSize::new(CHAT_MIN_WIDTH, CHAT_MIN_HEIGHT)));
        let _ = win.set_max_size(Some(LogicalSize::new(CHAT_MAX_WIDTH, 4000.0)));
    }
}

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

/// Ease-out cubic for window morph (fast start, soft land).
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn lerp(a: i32, b: i32, t: f32) -> i32 {
    (a as f32 + (b as f32 - a as f32) * t).round() as i32
}

/// Animates native window rect start -> end. Frame moves, content fades via CSS.
/// Unused now: per-frame set_position/set_size round-trips block on
/// Windows (~20-50ms each) and feel sluggish. Kept for tests.
#[allow(dead_code)]
fn animate_rect(win: &WebviewWindow, start: (i32, i32, u32, u32), end: (i32, i32, u32, u32)) {
    const FRAMES: i32 = 12;
    for i in 1..=FRAMES {
        let t = ease_out_cubic(i as f32 / FRAMES as f32);
        let x = lerp(start.0, end.0, t);
        let y = lerp(start.1, end.1, t);
        let w = lerp(start.2 as i32, end.2 as i32, t).max(1) as u32;
        let h = lerp(start.3 as i32, end.3 as i32, t).max(1) as u32;
        let _ = win.set_position(PhysicalPosition::new(x, y));
        let _ = win.set_size(PhysicalSize::new(w, h));
        if i < FRAMES {
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
    }
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

/// Top-left the window should sit at for `preset` (or the saved custom spot).
fn position_for(window: &WebviewWindow, preset: &str, custom: &WindowGeometry, size: PhysicalSize<u32>) -> Option<(i32, i32)> {
    if preset == "custom" && on_any_monitor(window, custom.x, custom.y) {
        return Some((custom.x, custom.y));
    }
    let monitor = window.current_monitor().ok().flatten().or_else(|| window.primary_monitor().ok().flatten())?;
    let area = monitor.work_area();
    Some(preset_position(preset, (area.position.x, area.position.y), (area.size.width, area.size.height), (size.width, size.height)))
}

pub fn apply_position(window: &WebviewWindow, preset: &str, custom: &WindowGeometry) {
    suppress_persistence();
    let Ok(size) = window.outer_size() else { return };
    if let Some((x, y)) = position_for(window, preset, custom, size) {
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

/// The chat window lives in the tray and must never own a taskbar button, but
/// Windows silently restores that button whenever the window style is rebuilt:
/// restoring it from minimized, toggling always-on-top, or showing it again
/// right after a hide. Re-assert the flag after any of those.
pub fn keep_off_taskbar(win: &WebviewWindow) {
    let _ = win.set_skip_taskbar(true);
}

/// Restores chat window size/always-on-top from settings and places the bubble.
pub fn restore(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings.get().general;
    if let Some(win) = main_window(app) {
        suppress_persistence();
        apply_size_bounds(&win, s.compact);
        if s.compact {
            let _ = win.set_size(LogicalSize::new(COMPACT_SIZE.0, COMPACT_SIZE.1));
        } else if s.window.width > 0 && s.window.height > 0 {
            let _ = win.set_size(PhysicalSize::new(s.window.width, s.window.height));
        }
        let _ = win.set_always_on_top(s.always_on_top);
        keep_off_taskbar(&win);
        apply_position(&win, &s.window_position, &s.window);
    }
    let _ = create_bubble(app);
}

pub fn create_bubble(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(BUBBLE) {
        return Ok(w);
    }
    let w = WebviewWindowBuilder::new(app, BUBBLE, WebviewUrl::App("index.html#/bubble".into()))
        .title("Open Local Assistant")
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

/// Bubble center relative to main origin, in logical (CSS) px for veil.
fn rel_logical(main_x: i32, main_y: i32, cx: i32, cy: i32, scale: f64) -> (f32, f32) {
    let s = if scale > 0.0 { scale } else { 1.0 };
    (((cx - main_x) as f64 / s) as f32, ((cy - main_y) as f64 / s) as f32)
}

/// Target chat size in physical px. Heals collapsed window (shrink anim
/// must never persist): falls back to saved settings, else 420x640 logical.
fn chat_target_size(win: &WebviewWindow, s: &crate::settings::Settings) -> PhysicalSize<u32> {
    let maximized = win.is_maximized().unwrap_or(false);
    if !maximized {
        if let Ok(sz) = win.outer_size() {
            if sz.width >= 240 && sz.height >= 240 {
                return sz;
            }
        }
    }
    if s.general.compact {
        let scale = win.scale_factor().unwrap_or(1.0);
        return PhysicalSize::new((COMPACT_SIZE.0 * scale).round() as u32, (COMPACT_SIZE.1 * scale).round() as u32);
    }
    let g = &s.general.window;
    if g.width >= 320 && g.height >= 260 {
        return PhysicalSize::new(g.width, g.height);
    }
    let scale = win.scale_factor().unwrap_or(1.0);
    PhysicalSize::new((420.0 * scale).round() as u32, (640.0 * scale).round() as u32)
}

pub fn hide_bubble(app: &AppHandle) {
    if let Some(b) = app.get_webview_window(BUBBLE) {
        let _ = b.hide();
    }
}

/// Opens chat anchored to bubble. Instant show + CSS fade.
/// No native resize loop: 24 window-manager ops block, feel sluggish.
pub fn show_main(app: &AppHandle, focus_input: bool) {
    let Some(win) = main_window(app) else { return };
    let s = app.state::<AppState>().settings.get().general;
    let mut origin = "bottom-right".to_string();
    let was_hidden = !win.is_visible().unwrap_or(false);
    // Capture bubble rect before hiding (zoom origin).
    let bubble_rect: Option<(i32, i32, u32, u32)> = app
        .get_webview_window(BUBBLE)
        .filter(|b| b.is_visible().unwrap_or(false))
        .and_then(|b| match (b.outer_position(), b.outer_size()) {
            (Ok(p), Ok(sz)) => Some((p.x, p.y, sz.width, sz.height)),
            _ => None,
        });
    let mut target: Option<(i32, i32, u32, u32)> = None;
    if was_hidden && s.window_position != "custom" {
        // A preset position is locked: the window always reopens exactly there
        // (only "custom" follows drags, see `on_main_window_event`).
        // Heal collapsed size from previous shrink (never persist tiny).
        let size = chat_target_size(&win, &app.state::<AppState>().settings.get());
        if let Ok(cur) = win.outer_size() {
            if cur.width != size.width || cur.height != size.height {
                suppress_persistence();
                let _ = win.set_size(size);
            }
        }
        if s.window_position == "bottom-left" {
            origin = "bottom-left".to_string();
        }
        target = position_for(&win, &s.window_position, &s.window, size).map(|(x, y)| (x, y, size.width, size.height));
    }
    if win.is_minimized().unwrap_or(false) {
        let _ = win.unminimize();
        keep_off_taskbar(&win);
    }
    let animated = bubble_rect.is_some() && was_hidden;
    // Bubble center in main logical px so shell zooms from bubble spot.
    // Custom-pos windows keep their place; origin outside box still gives
    // directional grow toward bubble.
    let scale = win.scale_factor().unwrap_or(1.0);
    let main_origin: (i32, i32) = target
        .map(|(tx, ty, _, _)| (tx, ty))
        .or_else(|| win.outer_position().map(|p| (p.x, p.y)).ok())
        .unwrap_or((0, 0));
    let (fx, fy) = match bubble_rect {
        Some((bx, by, bw, bh)) => rel_logical(main_origin.0, main_origin.1, bx + bw as i32 / 2, by + bh as i32 / 2, scale),
        _ => {
            // Fallback: origin corner of current window size.
            let sz = win.outer_size().unwrap_or(PhysicalSize::new(420, 640));
            let (w, h) = (sz.width as f64 / scale, sz.height as f64 / scale);
            match origin.as_str() {
                "bottom-left" => (24.0, (h - 24.0) as f32),
                "top-right" => ((w - 24.0) as f32, 24.0),
                _ => ((w - 24.0) as f32, (h - 24.0) as f32),
            }
        }
    };
    // Pre-arm BEFORE swapping: hidden webview still runs JS, so opening
    // state commits offscreen. First visible frame already mid-zoom at bubble
    // spot. Never a frame with both bubble + full chat, never stale veil.
    let _ = app.emit_to(MAIN, "app://window-shown", serde_json::json!({ "origin": origin, "animated": animated, "fx": fx, "fy": fy }));
    std::thread::sleep(std::time::Duration::from_millis(50));
    suppress_persistence();
    // Snap to anchor. Resize loop sluggish on Windows; shell zoom in webview
    // (GPU) gives continuity with zero native resize.
    if let Some((tx, ty, _, _)) = target {
        let _ = win.set_position(PhysicalPosition::new(tx, ty));
    }
    // Hide FIRST, same tick as show: never both visible.
    hide_bubble(app);
    let _ = win.show();
    let _ = win.set_focus();
    keep_off_taskbar(&win);
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

/// Hides chat, shows bubble. Emits closing veil first (190ms shrink),
/// then swaps windows so blob lands exactly on bubble. Instant hide_to_tray
/// unaffected. Heals collapsed size while hidden so next open full.
pub fn minimize_to_bubble(app: &AppHandle) {
    let Some(w) = main_window(app) else {
        show_bubble(app);
        return;
    };
    if let Ok(sz) = w.outer_size() {
        if sz.width < 240 || sz.height < 240 {
            let full = chat_target_size(&w, &app.state::<AppState>().settings.get());
            let _ = w.hide();
            suppress_persistence();
            let _ = w.set_size(full);
            show_bubble(app);
            return;
        }
    }
    // Bubble center in main logical px for shrink target. Bubble hidden but
    // retains last position.
    let (fx, fy) = match (w.outer_position(), w.outer_size(), app.get_webview_window(BUBBLE)) {
        (Ok(mp), Ok(_), Some(b)) => match (b.outer_position(), b.outer_size()) {
            (Ok(bp), Ok(bs)) => {
                let scale = w.scale_factor().unwrap_or(1.0);
                rel_logical(mp.x, mp.y, bp.x + bs.width as i32 / 2, bp.y + bs.height as i32 / 2, scale)
            }
            _ => (0.0, 0.0),
        },
        _ => (0.0, 0.0),
    };
    let _ = app.emit_to(MAIN, "app://window-closing", serde_json::json!({ "fx": fx, "fy": fy }));
    std::thread::sleep(std::time::Duration::from_millis(190));
    let _ = w.hide();
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
            // Maximizing moves the window to the screen corner; that is neither
            // a drag to remember nor a move away from a locked preset.
            if main_window(app).is_some_and(|w| w.is_maximized().unwrap_or(false)) {
                return;
            }
            let state = app.state::<AppState>();
            let g = state.settings.get().general;
            if g.window_position != "custom" {
                // A preset position is locked; only "custom" follows drags. Anything
                // that still moves it (Win+arrow, a stray drag) is undone.
                if let Some(win) = main_window(app) {
                    apply_position(&win, &g.window_position, &g.window);
                }
                return;
            }
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
            // The maximized size is temporary; restoring brings back the saved one.
            if main_window(app).is_some_and(|w| w.is_maximized().unwrap_or(false)) {
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

/// Maximizes or restores the chat window; works in every position mode,
/// a locked preset included. The width cap is lifted while maximized, and
/// restoring puts the window back at its preset or custom spot. Returns
/// whether the window is maximized now.
pub fn toggle_maximize(app: &AppHandle) -> bool {
    let Some(win) = main_window(app) else { return false };
    let s = app.state::<AppState>().settings.get();
    suppress_persistence();
    if win.is_maximized().unwrap_or(false) {
        let _ = win.unmaximize();
        apply_size_bounds(&win, s.general.compact);
        let g = &s.general.window;
        if !s.general.compact {
            let _ = win.set_size(PhysicalSize::new(g.width, g.height));
        }
        apply_position(&win, &s.general.window_position, g);
        false
    } else {
        if s.general.compact {
            return false;
        }
        let _ = win.set_max_size(None::<LogicalSize<f64>>);
        let _ = win.maximize();
        true
    }
}

pub fn set_compact(app: &AppHandle, compact: bool) {
    let state = app.state::<AppState>();
    if compact && main_window(app).is_some_and(|w| w.is_maximized().unwrap_or(false)) {
        toggle_maximize(app);
    }
    let Ok(s) = state.settings.update(|s| s.general.compact = compact) else { return };
    // Settings windows mirror this switch, so tell every window about it.
    let _ = app.emit("settings://changed", &s);
    let Some(win) = main_window(app) else { return };
    suppress_persistence();
    apply_size_bounds(&win, compact);
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

/// Creates the settings window hidden (idempotent). Called at startup:
/// on-demand creation flakes on some machines while startup-created
/// webviews always work.
pub fn ensure_settings(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        return Ok(w);
    }
    tracing::info!("open_settings creating new window");
    // Tall enough that the whole sidebar fits without scrolling, but never
    // taller than the screen (leaves room for the taskbar and title bar).
    let height = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| (m.size().height as f64 / m.scale_factor() - 120.0).max(520.0))
        .map_or(SETTINGS_H, |avail| avail.min(SETTINGS_H));
    let w = match WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("index.html#/settings/general".into()))
        .title("Open Local Assistant — Settings")
        .inner_size(1000.0, height)
        .min_inner_size(720.0, 520.0)
        .max_inner_size(1400.0, 1000.0)
        .center()
        // Transparent like every working window (bubble/overlay/main):
        // CSS paints opaque bg, so look identical when healthy.
        .transparent(true)
        .visible(false)
        .build()
    {
        Ok(w) => w,
        Err(e) => {
            tracing::error!(error = %e, "open_settings build failed");
            return Err(e);
        }
    };
    // Hide rather than destroy on close: rebuilding this window from scratch
    // can render blank (WebView2 re-creates the same label too quickly), and
    // destroying it while a shortcut is mid-recording would skip the cleanup
    // that re-registers global shortcuts, leaving them dead for the session.
    // register_all() here is a safety net for that second case regardless.
    let h = app.clone();
    w.on_window_event(move |e| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = e {
            api.prevent_close();
            super::shortcuts::register_all(&h);
            if let Some(w) = h.get_webview_window(SETTINGS) {
                let _ = w.hide();
            }
        }
    });
    Ok(w)
}

pub fn open_settings(app: &AppHandle, section: Option<&str>) -> tauri::Result<()> {
    let route = format!("index.html#/settings/{}", section.unwrap_or("general"));
    tracing::info!(route = %route, "open_settings requested");
    let existed = app.get_webview_window(SETTINGS).is_some();
    let w = ensure_settings(app)?;
    tracing::info!(existed, "open_settings window ready");
    let _ = app.emit_to(SETTINGS, "app://navigate", format!("/settings/{}", section.unwrap_or("general")));
    let _ = w.unminimize();
    if let Err(e) = w.show() {
        tracing::error!(error = %e, "open_settings show failed");
        return Err(e.into());
    }
    if let Err(e) = w.set_focus() {
        tracing::error!(error = %e, "open_settings focus failed");
        return Err(e.into());
    }
    // The window was built hidden at startup, so its taskbar button only exists
    // now; give it the accent icon again so it does not show up broken.
    super::icon::apply_accent_to(&w, &app.state::<AppState>().settings.get().general.accent);
    // TEMP PROBE (remove after diagnosis): if JS runs, body goes red under
    // the opaque UI (invisible when healthy). Red visible = nav+JS alive,
    // React missing. White = navigation dead.
    std::thread::sleep(std::time::Duration::from_millis(800));
    let _ = w.eval("document.body.style.background='#ff0000'");
    Ok(())
}

/// Logical size of the dictation overlay card. Listening and review share it,
/// so the card does not jump when a result opens for editing.
const OVERLAY_W: f64 = 480.0;
const OVERLAY_H: f64 = 176.0;
/// Gap between the card and the taskbar at the default spot (logical px).
const OVERLAY_BOTTOM_GAP: f64 = 6.0;

#[cfg(windows)]
fn hwnd_of(raw: isize) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(raw as *mut core::ffi::c_void)
}

/// The window that currently has keyboard focus (0 when unknown).
#[cfg(windows)]
fn foreground_window() -> isize {
    // SAFETY: GetForegroundWindow has no preconditions.
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow().0 as isize }
}

#[cfg(not(windows))]
fn foreground_window() -> isize {
    0
}

/// Brings a window to the foreground. Windows only lets the process that got
/// the last input do this, so the input queues are attached for the call.
#[cfg(windows)]
fn focus_hwnd(raw: isize) {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow};
    if raw == 0 {
        return;
    }
    let target = hwnd_of(raw);
    // SAFETY: plain Win32 calls on a handle that is validated with IsWindow.
    unsafe {
        if !IsWindow(Some(target)).as_bool() {
            return;
        }
        let (this, other) = (GetCurrentThreadId(), GetWindowThreadProcessId(GetForegroundWindow(), None));
        let attached = other != 0 && other != this && AttachThreadInput(this, other, true).as_bool();
        let _ = BringWindowToTop(target);
        let _ = SetForegroundWindow(target);
        if attached {
            let _ = AttachThreadInput(this, other, false);
        }
        let _ = SetFocus(Some(target));
    }
}

#[cfg(not(windows))]
fn focus_hwnd(_raw: isize) {}

fn overlay_hwnd(app: &AppHandle) -> isize {
    app.get_webview_window(OVERLAY).and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize).unwrap_or(0)
}

/// Remembers which window had focus when dictation started, so a reviewed
/// result can be typed back into it after the overlay took focus.
pub fn remember_target(app: &AppHandle) {
    let fg = foreground_window();
    if fg != 0 && fg != overlay_hwnd(app) {
        app.state::<AppState>().dictation_target.store(fg, Ordering::Relaxed);
    }
}

/// Gives keyboard focus back to the window dictation started in.
pub fn restore_target(app: &AppHandle) {
    focus_hwnd(app.state::<AppState>().dictation_target.load(Ordering::Relaxed));
}

/// Invisible room kept above and below the overlay card (logical px) so the
/// language menu can hang outside the card without the window ever moving or
/// resizing while it is on screen: a move always shows the card jumping for a
/// frame or two before the webview catches up. The room is cut away with a
/// window region (not drawn, clicks pass through); the open menu is added to
/// the region. Fits the menu's 8 rows plus its gap from the pill.
const MENU_ROOM: f64 = 240.0;

/// The open language menu's rectangle in the window (logical x, y, w, h).
static MENU_RECT: Mutex<Option<[f64; 4]>> = Mutex::new(None);
/// Card margin inside the window and corner radius, the menu's corner radius,
/// and the room left around the menu for its shadow (logical px; must match
/// .overlay-card / .overlay-lang-menu).
const CARD_MARGIN: f64 = 8.0;
const CARD_RADIUS: f64 = 18.0;
const MENU_RADIUS: f64 = 10.0;
const MENU_SHADOW: f64 = 14.0;

fn room_px(w: &WebviewWindow) -> i32 {
    (MENU_ROOM * w.scale_factor().unwrap_or(1.0)).round() as i32
}

/// Window size for a card of logical height `card_h`.
fn overlay_window_size(card_h: f64) -> LogicalSize<f64> {
    LogicalSize::new(OVERLAY_W, card_h + 2.0 * MENU_ROOM)
}

/// Frameless windows keep their caption styles (for snapping and animations)
/// and merely hide the frame. Once a window region is set, Windows stops
/// drawing the modern frame and paints the classic title bar and border
/// instead, so the overlay drops those styles before it gets a region. The
/// window library adds them back whenever it updates the window (showing it,
/// changing focusability), so this runs before every region change.
#[cfg(windows)]
fn strip_frame_styles(raw: isize) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_CAPTION,
        WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
    };
    let hwnd = hwnd_of(raw);
    let frame = (WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX).0 as isize;
    // SAFETY: plain Win32 calls on the overlay's own window handle.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let stripped = (style & !frame) | WS_POPUP.0 as isize;
        if stripped != style {
            SetWindowLongPtrW(hwnd, GWL_STYLE, stripped);
            let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }
}

/// Lets the overlay take keyboard focus (review) or not (listening, so the app
/// being dictated into keeps it). The window library's `set_focusable`
/// rewrites every window style and redraws the frame, which flashed the
/// classic Windows border around the clipped card for a frame; only the
/// "no activate" flag is flipped here. The library still sees the overlay as
/// not focusable, so its next style update (showing it) puts the flag back,
/// which is also what listening needs.
#[cfg(windows)]
fn set_overlay_activatable(app: &AppHandle, _w: &WebviewWindow, activatable: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE};
    let raw = overlay_hwnd(app);
    if raw == 0 {
        return;
    }
    let hwnd = hwnd_of(raw);
    // SAFETY: plain Win32 calls on the overlay's own window handle.
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let flag = WS_EX_NOACTIVATE.0 as isize;
        let next = if activatable { ex & !flag } else { ex | flag };
        if next != ex {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        }
    }
}

#[cfg(not(windows))]
fn set_overlay_activatable(_app: &AppHandle, w: &WebviewWindow, activatable: bool) {
    let _ = w.set_focusable(activatable);
}

/// Clips the overlay window to the card plus its shadow margin, and the open
/// language menu plus its shadow, each with rounded corners following the
/// element's own (`clip`); or removes the clip while the overlay is hidden.
/// Nothing else of the window is drawn or clickable: its transparent room
/// would otherwise show as a dim box and block clicks to the app behind.
#[cfg(windows)]
fn set_overlay_region(app: &AppHandle, w: &WebviewWindow, clip: bool) {
    use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, HGDIOBJ, RGN_OR};
    let raw = overlay_hwnd(app);
    let Ok(size) = w.outer_size() else { return };
    if raw == 0 {
        return;
    }
    if clip {
        strip_frame_styles(raw);
    }
    let scale = w.scale_factor().unwrap_or(1.0);
    let px = |v: f64| (v * scale).round() as i32;
    let room = room_px(w);
    let menu = *MENU_RECT.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: plain Win32 calls on the overlay's own window handle. The menu
    // region is merged into the card's and freed; on success the system owns
    // the combined region, so it is not freed here.
    unsafe {
        let region = clip.then(|| {
            // CreateRoundRectRgn takes the corner ellipse's size (2x radius) and
            // leaves out the right and bottom edges, hence +1.
            let d = px(CARD_RADIUS + CARD_MARGIN) * 2;
            let card = CreateRoundRectRgn(0, room, size.width as i32 + 1, size.height as i32 - room + 1, d, d);
            if let Some([x, y, mw, mh]) = menu {
                let s = MENU_SHADOW;
                let d = px(MENU_RADIUS + s) * 2;
                let rgn = CreateRoundRectRgn(px(x - s), px(y - s), px(x + mw + s) + 1, px(y + mh + s) + 1, d, d);
                CombineRgn(Some(card), Some(card), Some(rgn), RGN_OR);
                let _ = DeleteObject(HGDIOBJ(rgn.0));
            }
            card
        });
        let _ = SetWindowRgn(hwnd_of(raw), region, true);
    }
}

#[cfg(not(windows))]
fn set_overlay_region(_app: &AppHandle, _w: &WebviewWindow, _clip: bool) {}

/// Runs an overlay transition in one go on the main thread. Window changes
/// requested from another thread (the dictation hotkey, the dictation flow)
/// only run later on the main thread; mixed with the region changes made right
/// away, the window was briefly shown unclipped or with its frame styles back,
/// which flashes a classic Windows border around the card. On the main thread
/// the whole transition happens before the window is painted again (window
/// calls made there run at once instead of being queued).
fn on_main(app: &AppHandle, f: impl FnOnce() + Send + 'static) {
    // Fails only while the app is shutting down; the overlay no longer matters then.
    let _ = app.run_on_main_thread(f);
}

/// Adds the open language menu (`Some(logical x, y, w, h)` in the window) to
/// the visible part of the overlay, or takes it away again (`None`).
pub fn set_overlay_menu(app: &AppHandle, menu: Option<[f64; 4]>) {
    *MENU_RECT.lock().unwrap_or_else(|p| p.into_inner()) = menu;
    if let Some(w) = app.get_webview_window(OVERLAY) {
        let app2 = app.clone();
        on_main(app, move || set_overlay_region(&app2, &w, true));
    }
}

/// Switches the overlay between listening (it never takes focus, so the app
/// being dictated into keeps it) and review (focusable: the result is edited).
pub fn set_overlay_review(app: &AppHandle, review: bool) {
    let Ok(w) = create_overlay(app) else { return };
    let app2 = app.clone();
    on_main(app, move || overlay_review_now(&app2, &w, review));
}

fn overlay_review_now(app: &AppHandle, w: &WebviewWindow, review: bool) {
    suppress_persistence();
    *MENU_RECT.lock().unwrap_or_else(|p| p.into_inner()) = None;
    set_overlay_activatable(app, w, review);
    // The open language menu (if any) was dropped from the visible area above.
    set_overlay_region(app, w, true);
    if review {
        focus_hwnd(overlay_hwnd(app));
        let _ = w.set_focus();
    }
}

/// Drops a result that is waiting for review and hands focus back. Returns
/// whether there was one.
pub fn discard_review(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let had = state.dictation_review.lock().unwrap_or_else(|p| p.into_inner()).take().is_some();
    if had {
        set_overlay_review(app, false);
        restore_target(app);
        state.dictation_busy.store(false, Ordering::Relaxed);
    }
    had
}

pub fn create_overlay(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(w) = app.get_webview_window(OVERLAY) {
        return Ok(w);
    }
    let w = WebviewWindowBuilder::new(app, OVERLAY, WebviewUrl::App("index.html#/overlay".into()))
        .title("Dictation")
        .inner_size(OVERLAY_W, OVERLAY_H + 2.0 * MENU_ROOM)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()?;
    // Suppress before attaching the listener: window construction itself can
    // fire a Moved event at some OS-default position, which must not get
    // persisted as if the user had dragged it there.
    suppress_persistence();
    let h = app.clone();
    let w2 = w.clone();
    w.on_window_event(move |e| {
        if let tauri::WindowEvent::Moved(pos) = e {
            if now_ms() < SUPPRESS_UNTIL.load(Ordering::Relaxed) {
                return;
            }
            // Saved as the card's spot, not the window's (which includes the room above).
            let (x, y) = (pos.x, pos.y + room_px(&w2));
            let _ = h.state::<AppState>().settings.update(|s| {
                s.dictation.overlay_x = Some(x);
                s.dictation.overlay_y = Some(y);
            });
        }
    });
    Ok(w)
}

/// Top-left of the card for the overlay's default centered-near-the-bottom
/// placement.
fn default_overlay_position(w: &WebviewWindow) -> Option<(i32, i32)> {
    let m = w.primary_monitor().ok().flatten()?;
    let area = m.work_area();
    let scale = m.scale_factor();
    let (cw, ch) = ((OVERLAY_W * scale).round() as i32, (OVERLAY_H * scale).round() as i32);
    let x = area.position.x + (area.size.width as i32 - cw) / 2;
    // The visible card sits CARD_MARGIN inside its box; leave only a small gap
    // between it and the taskbar.
    let y = area.position.y + area.size.height as i32 - ch + ((CARD_MARGIN - OVERLAY_BOTTOM_GAP) * scale).round() as i32;
    Some((x, y))
}

/// Shows the dictation overlay at its last dragged spot, or centered near the
/// bottom without taking focus if it was never dragged (or that spot's
/// monitor is no longer connected).
pub fn show_overlay(app: &AppHandle) {
    let Ok(w) = create_overlay(app) else { return };
    let app2 = app.clone();
    on_main(app, move || show_overlay_now(&app2, &w));
}

/// Keeps a saved card spot fully inside its monitor's work area (the card may
/// have been dragged there when it was smaller, or on another resolution).
fn card_on_screen(w: &WebviewWindow, x: i32, y: i32) -> (i32, i32) {
    let Ok(monitors) = w.available_monitors() else { return (x, y) };
    let Some(m) = monitors.iter().find(|m| {
        let (p, s) = (m.position(), m.size());
        x >= p.x && y >= p.y && x < p.x + s.width as i32 && y < p.y + s.height as i32
    }) else {
        return (x, y);
    };
    let scale = m.scale_factor();
    let size = ((OVERLAY_W * scale).round() as u32, (OVERLAY_H * scale).round() as u32);
    let area = m.work_area();
    clamp_into((area.position.x, area.position.y), (area.size.width, area.size.height), (x, y), size)
}

fn show_overlay_now(app: &AppHandle, w: &WebviewWindow) {
    let saved = app.state::<AppState>().settings.get().dictation;
    suppress_persistence();
    *MENU_RECT.lock().unwrap_or_else(|p| p.into_inner()) = None;
    let _ = w.set_size(overlay_window_size(OVERLAY_H));
    let pos = match (saved.overlay_x, saved.overlay_y) {
        (Some(x), Some(y)) if on_any_monitor(w, x, y) => Some(card_on_screen(w, x, y)),
        _ => default_overlay_position(w),
    };
    if let Some((x, y)) = pos {
        let _ = w.set_position(PhysicalPosition::new(x, y - room_px(w)));
    }
    let _ = w.show();
    // After showing: that re-adds the frame styles the region needs gone.
    set_overlay_region(app, w, true);
}

/// Clears the dragged overlay position and moves it back to the default spot
/// immediately (even while hidden).
pub fn reset_overlay_position(app: &AppHandle) {
    let _ = app.state::<AppState>().settings.update(|s| {
        s.dictation.overlay_x = None;
        s.dictation.overlay_y = None;
    });
    let Ok(w) = create_overlay(app) else { return };
    suppress_persistence();
    if let Some((x, y)) = default_overlay_position(&w) {
        let _ = w.set_position(PhysicalPosition::new(x, y - room_px(&w)));
    }
}

pub fn hide_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(OVERLAY) {
        let app2 = app.clone();
        on_main(app, move || {
            let _ = w.hide();
            // Showing it again changes its styles; that must not happen while clipped.
            set_overlay_region(&app2, &w, false);
        });
    }
}

/// Closes the overlay right away (Esc / overlay X). Whatever is in flight is
/// dropped: listening stops without inserting, a result waiting for review is
/// discarded, and a transcript still being transcribed or corrected is
/// cancelled when it arrives. Closing an overlay that only shows a finished
/// result ("Inserted", an error) just hides it.
pub fn cancel_dictation(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !discard_review(app) {
        // Also covers the transcribing/correcting stretch, when nothing is
        // listening yet run_dictation still has to see the cancel. A stale flag
        // is harmless: the next session clears it on start.
        state.dictation_cancel.store(true, Ordering::Relaxed);
        if state.voice.active_mode() == Some(ListenMode::Dictation) {
            state.voice.stop(true);
        }
    }
    hide_overlay(app);
}

#[cfg(test)]
mod tests {
    use super::{clamp_into, ease_out_cubic, lerp, preset_position, rel_logical};

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

    #[test]
    fn morph_easing_monotonic() {
        assert!((ease_out_cubic(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-6);
        let a = ease_out_cubic(0.25);
        let b = ease_out_cubic(0.75);
        assert!(a > 0.25 && b > a && b < 1.0);
        assert_eq!(lerp(0, 100, 0.0), 0);
        assert_eq!(lerp(0, 100, 1.0), 100);
        assert_eq!(lerp(84, 420, 0.5), 252);
    }

    #[test]
    fn veil_point_tracks_bubble_center() {
        // Main at (100,100) phys, bubble center (500,700) phys, scale 2.
        let (fx, fy) = rel_logical(100, 100, 500, 700, 2.0);
        assert!((fx - 200.0).abs() < 1e-6);
        assert!((fy - 300.0).abs() < 1e-6);
        // Zero scale falls back to 1.
        assert_eq!(rel_logical(0, 0, 60, 80, 0.0), (60.0, 80.0));
    }
}
