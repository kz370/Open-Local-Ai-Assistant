//! Accent-colored tray + taskbar/window icons.
//!
//! The bubble follows the app theme via CSS vars; the native tray and window
//! icons are static PNGs by default, so we generate a small accent-tinted
//! circle+glyph at runtime and push it to the tray + visible windows.

use tauri::{image::Image, Manager};

fn accent_rgb(name: &str) -> (u8, u8, u8) {
    match name {
        "blue" => (37, 99, 235),
        "green" => (22, 163, 74),
        "amber" => (217, 119, 6),
        "rose" => (225, 29, 72),
        "slate" => (71, 85, 105),
        _ => (20, 184, 166), // teal default
    }
}

fn mix(c: (u8, u8, u8), white: bool, amt: f32) -> (u8, u8, u8) {
    let t = if white { 255.0 } else { 0.0 };
    let m = |v: u8| ((v as f32) * (1.0 - amt) + t * amt).round().clamp(0.0, 255.0) as u8;
    (m(c.0), m(c.1), m(c.2))
}

/// 64x64 RGBA: accent circle with subtle vertical gradient + white wave bars.
fn render_accent_icon(accent: &str) -> Image<'static> {
    const S: u32 = 64;
    let base = accent_rgb(accent);
    let mut rgba = vec![0u8; (S * S * 4) as usize];
    let cx = S as f32 / 2.0;
    let cy = S as f32 / 2.0;
    let r = 28.0;
    for y in 0..S {
        for x in 0..S {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            if d > r {
                continue;
            }
            // Vertical gradient: lighten top, darken bottom.
            let t = y as f32 / S as f32; // 0 top -> 1 bottom
            let shade = 0.16 - t * 0.32; // +0.16 .. -0.16
            let (rr, gg, bb) = if shade >= 0.0 { mix(base, true, shade) } else { mix(base, false, -shade) };
            // Soft edge AA on outer 1px.
            let alpha = if d > r - 1.0 { ((r - d) * 255.0).round().clamp(0.0, 255.0) as u8 } else { 255 };
            let i = ((y * S + x) * 4) as usize;
            rgba[i] = rr;
            rgba[i + 1] = gg;
            rgba[i + 2] = bb;
            rgba[i + 3] = alpha;
        }
    }
    // White mini wave: 5 vertical bars centered.
    let bars = [(-14.0, 6.0), (-7.0, 14.0), (0.0, 22.0), (7.0, 14.0), (14.0, 8.0)];
    for (bx, h) in bars {
        let x0 = (cx + bx - 2.0).round() as i32;
        let x1 = (cx + bx + 2.0).round() as i32;
        let y0 = (cy - h / 2.0).round() as i32;
        let y1 = (cy + h / 2.0).round() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                if x < 0 || y < 0 || x >= S as i32 || y >= S as i32 {
                    continue;
                }
                let i = ((y as u32 * S + x as u32) * 4) as usize;
                rgba[i] = 255;
                rgba[i + 1] = 255;
                rgba[i + 2] = 255;
                rgba[i + 3] = 255;
            }
        }
    }
    Image::new_owned(rgba, S, S)
}

/// Push accent icon to tray + all windows. Failures only warn.
pub fn apply_accent(app: &tauri::AppHandle, accent: &str) {
    let icon = render_accent_icon(accent);
    // Tray (taskbar overflow on Windows).
    if let Some(tray) = app.tray_by_id("main") {
        // set_icon takes Option<Image>; clone via fresh render (cheap 64px).
        if let Err(e) = tray.set_icon(Some(render_accent_icon(accent))) {
            tracing::warn!(error = %e, "tray icon tint failed");
        }
    }
    // Windows (taskbar icon for settings, alt-tab, etc).
    for w in app.webview_windows().values() {
        let _ = w.set_icon(render_accent_icon(accent));
    }
    let _ = icon;
}

#[cfg(test)]
mod tests {
    use super::{accent_rgb, render_accent_icon};

    #[test]
    fn accent_map_covers_all() {
        for (name, expect) in [
            ("teal", (20, 184, 166)),
            ("blue", (37, 99, 235)),
            ("green", (22, 163, 74)),
            ("amber", (217, 119, 6)),
            ("rose", (225, 29, 72)),
            ("slate", (71, 85, 105)),
        ] {
            assert_eq!(accent_rgb(name), expect);
        }
        // Unknown falls back to teal.
        assert_eq!(accent_rgb("nope"), (20, 184, 166));
    }

    #[test]
    fn icon_is_64_rgba_with_visible_pixels() {
        let img = render_accent_icon("teal");
        assert_eq!((img.width(), img.height()), (64, 64));
        let rgba = img.rgba();
        assert_eq!(rgba.len(), 64 * 64 * 4);
        // Center pixel opaque (inside circle, possibly white glyph).
        let c = ((32 * 64 + 32) * 4) as usize;
        assert_eq!(rgba[c + 3], 255);
        // Corner pixel transparent (outside circle).
        assert_eq!(rgba[3], 0);
    }
}
