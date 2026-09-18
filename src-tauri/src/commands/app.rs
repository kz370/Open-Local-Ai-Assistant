use super::CmdResult;
use crate::capabilities::CapabilityReport;
use crate::desktop::{icon, shortcuts, window};
use crate::errors::AppError;
use crate::services::ai::model_selector::ModelSelection;
use crate::services::ai::{AiService, ConnectionStatus, ModelInfo};
use crate::services::hardware::HardwareInfo;
use crate::settings::Settings;
use crate::state::{AppPaths, AppState};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.get()
}

/// Saves settings and applies side effects that depend on changed values.
#[tauri::command]
pub async fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> CmdResult<Settings> {
    let before = state.settings.get();
    let saved = state.settings.set(settings)?;

    if before.ai.server_url != saved.ai.server_url {
        state.lmstudio.set_base_url(&saved.ai.server_url);
        state.resolver.invalidate().await;
    }
    if before.ai.api_key != saved.ai.api_key {
        state.lmstudio.set_api_key(saved.ai.api_key.clone());
        state.resolver.invalidate().await;
    }
    if before.ai.request_timeout_secs != saved.ai.request_timeout_secs {
        state.lmstudio.set_timeout(saved.ai.request_timeout_secs);
    }
    if before.stt.extra_model_dirs != saved.stt.extra_model_dirs {
        state.models.set_extra_dirs(saved.stt.extra_model_dirs.iter().map(std::path::PathBuf::from).collect());
        let _ = app.emit("models://changed", ());
    }
    if before.stt.model != saved.stt.model
        || before.stt.language != saved.stt.language
        || before.stt.extra_model_dirs != saved.stt.extra_model_dirs
        || before.stt.silence_ms != saved.stt.silence_ms
    {
        state.stt.unload();
    }
    if state.captions.is_active() && crate::services::captions::LiveCaptions::needs_restart(&before, &saved) {
        if let Err(e) = state.captions.start() {
            tracing::warn!(error = %e, "live captions could not restart with the new settings");
            let _ = app.emit("captions://event", serde_json::json!({ "type": "error", "code": e.code(), "detail": e.to_string() }));
        }
    }
    if before.tts.output_device != saved.tts.output_device || before.tts.volume != saved.tts.volume {
        state.tts.apply_settings();
    }
    let g_before = &before.general;
    let g = &saved.general;
    if g_before.global_shortcut != g.global_shortcut
        || g_before.push_to_talk_shortcut != g.push_to_talk_shortcut
        || before.stt.push_to_talk != saved.stt.push_to_talk
        || before.dictation.enabled != saved.dictation.enabled
        || before.dictation.shortcut != saved.dictation.shortcut
        || before.captions.shortcut != saved.captions.shortcut
    {
        shortcuts::register_all(&app);
    }
    if g_before.start_with_os != g.start_with_os {
        let al = app.autolaunch();
        let res = if g.start_with_os { al.enable() } else { al.disable() };
        if let Err(e) = res {
            tracing::warn!(error = %e, "autostart change failed");
        }
    }
    if let Some(win) = window::main_window(&app) {
        if g_before.always_on_top != g.always_on_top {
            let _ = win.set_always_on_top(g.always_on_top);
            window::keep_off_taskbar(&win);
        }
        if g_before.window_position != g.window_position && g.window_position != "custom" {
            window::apply_position(&win, &g.window_position, &g.window);
        }
    }
    if g_before.accent != g.accent {
        icon::apply_accent(&app, &g.accent);
    }
    let _ = app.emit("settings://changed", &saved);
    Ok(saved)
}

/// While the user records a shortcut, global shortcuts must not swallow the keys.
#[tauri::command]
pub fn shortcuts_capture(app: AppHandle, capturing: bool) {
    if capturing {
        shortcuts::unregister_all(&app);
    } else {
        shortcuts::register_all(&app);
    }
}

#[tauri::command]
pub fn shortcut_errors(state: State<'_, AppState>) -> Vec<String> {
    state.shortcut_errors.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub paths: AppPaths,
    pub hardware: HardwareInfo,
    pub os: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle, state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        paths: state.paths.clone(),
        hardware: state.hardware.clone(),
        os: std::env::consts::OS.into(),
    }
}

fn open_path(path: &std::path::Path) -> CmdResult<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(windows)]
    let cmd = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = std::process::Command::new("xdg-open").arg(path).spawn();
    cmd.map(|_| ()).map_err(|e| AppError::Io(e.to_string()))
}

#[tauri::command]
pub fn open_folder(state: State<'_, AppState>, which: String) -> CmdResult<()> {
    let p = match which.as_str() {
        "logs" => &state.paths.logs_dir,
        "models" => &state.paths.models_dir,
        "data" => &state.paths.data_dir,
        _ => return Err(AppError::Invalid("unknown folder".into())),
    };
    open_path(p)
}

/// Last lines of today's log file (technical details for Diagnostics).
#[tauri::command]
pub fn read_log_tail(state: State<'_, AppState>, lines: Option<usize>) -> CmdResult<String> {
    let mut files: Vec<_> = std::fs::read_dir(&state.paths.logs_dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "log")).collect();
    files.sort();
    let Some(latest) = files.last() else { return Ok(String::new()) };
    let text = std::fs::read_to_string(latest)?;
    let n = lines.unwrap_or(200).min(2000);
    let all: Vec<&str> = text.lines().collect();
    Ok(all[all.len().saturating_sub(n)..].join("\n"))
}

#[tauri::command]
pub async fn scan_capabilities(state: State<'_, AppState>) -> CmdResult<CapabilityReport> {
    let settings = state.settings.get();
    Ok(state.capabilities().scan(&settings).await)
}

#[tauri::command]
pub async fn lmstudio_test(state: State<'_, AppState>, url: Option<String>, api_key: Option<String>) -> CmdResult<ConnectionStatus> {
    let url_changed = url.as_deref().is_some_and(|u| u.trim() != state.lmstudio.base_url());
    let key_changed = api_key != state.lmstudio.api_key();
    if !url_changed && !key_changed {
        return state.lmstudio.test_connection().await;
    }
    let probe = crate::services::ai::lmstudio::LmStudioService::new(url.as_deref().unwrap_or(&state.lmstudio.base_url()), 10);
    probe.set_api_key(if key_changed { api_key } else { state.lmstudio.api_key() });
    probe.test_connection().await
}

#[tauri::command]
pub async fn lmstudio_models(state: State<'_, AppState>, refresh: bool) -> CmdResult<Vec<ModelInfo>> {
    state.resolver.models(refresh).await
}

#[tauri::command]
pub async fn lmstudio_auto_selection(state: State<'_, AppState>) -> CmdResult<Option<ModelSelection>> {
    state.resolver.auto_selection().await
}

#[tauri::command]
pub fn window_set_compact(app: AppHandle, compact: bool) {
    window::set_compact(&app, compact);
}

/// Closes the assistant to the system tray (all windows hidden).
#[tauri::command]
pub fn window_hide(app: AppHandle) {
    window::hide_to_tray(&app);
}

/// Called by the bubble when clicked.
#[tauri::command]
pub fn bubble_open_chat(app: AppHandle) {
    window::show_main(&app, true);
}

/// Minimizes the chat window to the floating bubble.
#[tauri::command]
pub fn window_minimize(app: AppHandle) {
    window::minimize_to_bubble(&app);
}

#[tauri::command]
pub fn window_show_main(app: AppHandle) {
    window::show_main(&app, true);
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle, section: Option<String>) -> CmdResult<()> {
    // Call DIRECTLY (worker thread): Tauri window ops self-dispatch to the
    // main loop internally. Routing build() onto main first self-deadlocks:
    // its inner main-loop rendezvous waits on the thread it already owns.
    tracing::info!(section = ?section, "open_settings_window command invoked");
    window::open_settings(&app, section.as_deref())?;
    Ok(())
}

#[tauri::command]
pub fn complete_first_run(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.settings.update(|s| s.general.first_run_complete = true)?;
    shortcuts::register_all(&app);
    window::show_main(&app, true);
    let _ = app.emit("settings://changed", state.settings.get());
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    crate::desktop::tray::quit(&app);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyStatus {
    pub llm: String,
    pub llm_server: String,
    pub llm_is_local_address: bool,
    pub stt_local: bool,
    pub tts_local: bool,
    pub conversations_local: bool,
    pub settings_local: bool,
    pub telemetry: bool,
    pub internet_via_mcp: bool,
    pub internet_servers: Vec<String>,
}

#[tauri::command]
pub async fn privacy_status(state: State<'_, AppState>) -> CmdResult<PrivacyStatus> {
    let url = state.lmstudio.base_url();
    let host = url::Url::parse(&url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
    let local = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1" | "[::1]") || host.starts_with("192.168.") || host.starts_with("10.") || host.ends_with(".local");
    let statuses = state.mcp.statuses().await?;
    let internet_servers: Vec<String> = statuses.iter().filter(|s| s.config.enabled && s.internet).map(|s| s.config.name.clone()).collect();
    Ok(PrivacyStatus {
        llm: "LM Studio".into(),
        llm_server: url,
        llm_is_local_address: local,
        stt_local: true,
        tts_local: true,
        conversations_local: true,
        settings_local: true,
        telemetry: false,
        internet_via_mcp: !internet_servers.is_empty(),
        internet_servers,
    })
}

#[tauri::command]
pub fn app_ready(app: AppHandle) {
    // The main window is created hidden and shown once the UI rendered, to avoid a white flash.
    let state = app.state::<AppState>();
    let s = state.settings.get().general;
    if !s.first_run_complete {
        window::show_main(&app, false);
    } else if !s.start_minimized {
        // The bubble is the resting state; "start minimized" keeps only the tray icon.
        window::show_bubble(&app);
    }
}
