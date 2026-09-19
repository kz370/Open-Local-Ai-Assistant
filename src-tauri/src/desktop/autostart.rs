//! "Start with Windows".
//!
//! The Tauri autostart plugin registers `current_exe()`, which is wrong while
//! the GPU pack is active: that process is a copy of the app running from the
//! pack folder in the app data directory (see `services::gpu`). Registering it
//! would make Windows start that copy, which is never updated with the app and
//! stops working when the pack is removed. The entry always points at the
//! installed executable instead.

use crate::errors::{AppError, AppResult};
use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use tauri::{AppHandle, Runtime};

/// Passed to the app when Windows starts it.
const ARGS: [&str; 1] = ["--minimized"];

fn launcher<R: Runtime>(app: &AppHandle<R>) -> AppResult<AutoLaunch> {
    let exe = crate::services::gpu::launcher_exe();
    AutoLaunchBuilder::new()
        .set_app_name(&app.package_info().name)
        .set_app_path(&exe.display().to_string())
        .set_args(&ARGS)
        .set_use_launch_agent(true)
        .build()
        .map_err(|e| AppError::Other(e.to_string()))
}

pub fn set_enabled<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> AppResult<()> {
    let al = launcher(app)?;
    let res = if enabled { al.enable() } else { al.disable() };
    res.map_err(|e| AppError::Other(e.to_string()))
}

/// Rewrites the entry at startup, so one written by an older version (or from
/// the GPU copy) points at the installed executable again.
pub fn sync<R: Runtime>(app: &AppHandle<R>, enabled: bool) {
    if !enabled {
        return;
    }
    if let Err(e) = set_enabled(app, true) {
        tracing::warn!(error = %e, "could not refresh the autostart entry");
    }
}
