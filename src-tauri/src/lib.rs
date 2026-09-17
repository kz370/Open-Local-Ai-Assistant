//! Local Assistant — a local-first desktop assistant powered by LM Studio.

pub mod capabilities;
pub mod commands;
pub mod database;
pub mod desktop;
pub mod errors;
pub mod logging;
pub mod services;
pub mod settings;
pub mod state;

use database::Db;
use desktop::{icon, shortcuts, tray, window};
use services::ai::lmstudio::LmStudioService;
use services::chat::{ChatEngine, ModelResolver};
use services::dictation;
use services::mcp::McpManager;
use services::models::ModelStore;
use services::stt::session::{ListenMode, VoiceEvent, VoiceSessions};
use services::stt::SttService;
use services::tts::TtsService;
use settings::SettingsStore;
use state::{AppPaths, AppState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

fn init_state(app: &AppHandle) -> Result<AppState, Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    let paths = AppPaths {
        models_dir: data_dir.join("models"),
        logs_dir: app.path().app_log_dir().unwrap_or_else(|_| data_dir.join("logs")),
        database: data_dir.join("assistant.db"),
        data_dir,
    };
    std::fs::create_dir_all(&paths.data_dir)?;

    let db = Arc::new(Db::open(&paths.database)?);
    let settings = Arc::new(SettingsStore::load(db.clone())?);
    let s = settings.get();
    let hardware = services::hardware::detect();
    tracing::info!(cpu = %hardware.cpu_name, cores = hardware.physical_cores, ram_gb = hardware.total_ram_bytes / (1 << 30), gpus = hardware.gpus.len(), "hardware detected");

    let lmstudio = Arc::new(LmStudioService::new(&s.ai.server_url, s.ai.request_timeout_secs));
    let resolver = Arc::new(ModelResolver::with_hardware(lmstudio.clone(), hardware.clone()));
    let models = Arc::new(ModelStore::new(paths.models_dir.clone()));
    models.set_extra_dirs(s.stt.extra_model_dirs.iter().map(std::path::PathBuf::from).collect());

    let handle = app.clone();
    let mcp = Arc::new(McpManager::new(db.clone(), Arc::new(move || {
        let _ = handle.emit("mcp://changed", ());
    })));

    let handle = app.clone();
    let tts = TtsService::new(models.clone(), settings.clone(), hardware.clone(), Arc::new(move |ev| {
        let _ = handle.emit("tts://event", ev);
    }));

    let stt = Arc::new(SttService::new(models.clone(), hardware.clone()));

    let handle = app.clone();
    let tts_probe = tts.clone();
    let voice = Arc::new(VoiceSessions::new(
        stt.clone(),
        settings.clone(),
        Arc::new(move |ev: VoiceEvent| on_voice_event(&handle, ev)),
        Arc::new(move || tts_probe.is_speaking()),
    ));

    let chat = Arc::new(ChatEngine::new(db.clone(), settings.clone(), lmstudio.clone(), resolver.clone(), mcp.clone(), tts.clone()));

    Ok(AppState {
        paths,
        db,
        settings,
        lmstudio,
        resolver,
        chat,
        mcp,
        models,
        stt,
        tts,
        voice,
        hardware,
        downloads: Mutex::new(Default::default()),
        dictation_busy: AtomicBool::new(false),
        dictation_cancel: AtomicBool::new(false),
        shortcut_errors: Mutex::new(Vec::new()),
    })
}

/// Routes voice events: dictation transcripts are handled natively, everything
/// else is forwarded to the UI.
fn on_voice_event(app: &AppHandle, ev: VoiceEvent) {
    match &ev {
        VoiceEvent::Transcript { mode: ListenMode::Dictation, text, .. } => {
            let text = text.clone();
            let app = app.clone();
            tauri::async_runtime::spawn(async move { run_dictation(app, text).await });
        }
        VoiceEvent::State { mode: ListenMode::Dictation, state, .. } => {
            // A stop arriving after Esc-cancel must not resurrect the overlay
            // as idle; report cancelled instead (flag stays for run_dictation).
            if state == "idle" && app.state::<AppState>().dictation_cancel.load(Ordering::Relaxed) {
                let _ = app.emit("dictation://state", serde_json::json!({ "state": "cancelled" }));
            } else {
                let _ = app.emit("dictation://state", serde_json::json!({ "state": state }));
            }
        }
        VoiceEvent::Error { mode: ListenMode::Dictation, code, detail } => {
            let _ = app.emit("dictation://state", serde_json::json!({ "state": "error", "error": { "code": code, "detail": detail } }));
            hide_overlay_later(app);
        }
        _ => {}
    }
    // Dictation feedback (levels and live partial text) goes to the overlay only.
    if matches!(&ev, VoiceEvent::Level { mode: ListenMode::Dictation, .. } | VoiceEvent::Partial { mode: ListenMode::Dictation, .. }) {
        let _ = app.emit_to(window::OVERLAY, "voice://event", ev);
        return;
    }
    let _ = app.emit("voice://event", ev);
}

fn hide_overlay_later(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
        let state = app.state::<AppState>();
        if state.voice.active_mode() != Some(ListenMode::Dictation) && !state.dictation_busy.load(Ordering::Relaxed) {
            window::hide_overlay(&app);
        }
    });
}

async fn run_dictation(app: AppHandle, raw: String) {
    let state = app.state::<AppState>();
    state.dictation_busy.store(true, Ordering::Relaxed);
    // Esc/X cancel wins over any pending transcript or correction.
    if state.dictation_cancel.swap(false, Ordering::Relaxed) {
        let _ = app.emit("dictation://state", serde_json::json!({ "state": "cancelled" }));
        state.dictation_busy.store(false, Ordering::Relaxed);
        return;
    }
    let settings = state.settings.get().dictation;
    let raw = raw.trim().to_string();
    let mut text = raw.clone();
    let mut corrected = false;
    let mut correction_error = None;
    if raw.is_empty() {
        let _ = app.emit("dictation://state", serde_json::json!({ "state": "empty" }));
    } else {
        if settings.correction_enabled {
            match settings.correction_model.as_deref().filter(|m| !m.is_empty()) {
                Some(model) => {
                    let _ = app.emit("dictation://state", serde_json::json!({ "state": "correcting" }));
                    match dictation::correct_text(state.lmstudio.as_ref(), model, &raw).await {
                        Ok(c) => {
                            text = c;
                            corrected = true;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "dictation correction failed; inserting raw transcript");
                            correction_error = Some(e);
                        }
                    }
                }
                None => correction_error = Some(errors::AppError::NoModel),
            }
        }
        // Cancel may have landed during the (slow) correction call.
        if state.dictation_cancel.swap(false, Ordering::Relaxed) {
            let _ = app.emit("dictation://state", serde_json::json!({ "state": "cancelled" }));
            state.dictation_busy.store(false, Ordering::Relaxed);
            return;
        }
        let final_text = dictation::finalize_text(&text, &settings);
        let s2 = settings.clone();
        let insert = tokio::task::spawn_blocking(move || dictation::insert_text(&final_text, &s2)).await;
        match insert {
            Ok(Ok(())) => {
                let result = dictation::DictationResult { raw: raw.clone(), inserted: text.clone(), corrected, correction_error: correction_error.map(|e| e.to_string()) };
                let _ = app.emit("dictation://state", serde_json::json!({ "state": "inserted", "result": result }));
            }
            Ok(Err(e)) => {
                let _ = app.emit("dictation://state", serde_json::json!({ "state": "error", "error": e }));
            }
            Err(e) => {
                let _ = app.emit("dictation://state", serde_json::json!({ "state": "error", "error": { "code": "other", "detail": e.to_string() } }));
            }
        }
    }
    state.dictation_busy.store(false, Ordering::Relaxed);
    hide_overlay_later(&app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Only one instance runs: a second launch focuses the existing assistant.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tracing::info!("second instance launched; focusing the existing window");
            window::show_main(app, true);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(shortcuts::handle).build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let log_dir = handle.path().app_log_dir().unwrap_or_else(|_| std::env::temp_dir().join("local-assistant-logs"));
            if let Some(guard) = logging::init(&log_dir) {
                // Keep the non-blocking writer alive for the whole process.
                Box::leak(Box::new(guard));
            }
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting Local Assistant");

            let state = init_state(&handle)?;
            let mcp = state.mcp.clone();
            app.manage(state);

            tray::create(&handle)?;
            shortcuts::register_all(&handle);
            window::restore(&handle);
            let _ = window::create_overlay(&handle);
            // Pre-create settings hidden: on-demand creation flakes on some
            // machines while startup-created webviews always work.
            let _ = window::ensure_settings(&handle);
            // Tint tray + taskbar icons to match saved accent.
            {
                let accent = handle.state::<AppState>().settings.get().general.accent.clone();
                icon::apply_accent(&handle, &accent);
            }

            if let Some(main) = window::main_window(&handle) {
                let h = handle.clone();
                main.on_window_event(move |event| window::on_main_window_event(&h, event));
            }

            // Debug helper: LA_OPEN=chat|settings opens that window at startup.
            match std::env::var("LA_OPEN").ok().as_deref() {
                Some("chat") => window::show_main(&handle, true),
                Some(other) if other.starts_with("settings") => {
                    let section = other.split_once(':').map(|(_, s)| s.to_string());
                    let _ = window::open_settings(&handle, section.as_deref());
                }
                _ => {}
            }

            tauri::async_runtime::spawn(async move { mcp.connect_enabled().await });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_settings,
            commands::app::save_settings,
            commands::app::shortcut_errors,
            commands::app::shortcuts_capture,
            commands::app::app_info,
            commands::app::open_folder,
            commands::app::read_log_tail,
            commands::app::scan_capabilities,
            commands::app::lmstudio_test,
            commands::app::lmstudio_models,
            commands::app::lmstudio_auto_selection,
            commands::app::window_set_compact,
            commands::app::window_hide,
            commands::app::window_minimize,
            commands::app::window_show_main,
            commands::app::bubble_open_chat,
            commands::app::open_settings_window,
            commands::app::complete_first_run,
            commands::app::quit_app,
            commands::app::privacy_status,
            commands::app::app_ready,
            commands::chat::chat_send,
            commands::chat::chat_stop,
            commands::chat::chat_confirm_tool,
            commands::chat::conv_list,
            commands::chat::conv_search,
            commands::chat::conv_get,
            commands::chat::conv_rename,
            commands::chat::conv_delete,
            commands::chat::conv_set_last,
            commands::chat::conv_export,
            commands::chat::conv_import,
            commands::mcp::mcp_list,
            commands::mcp::mcp_save,
            commands::mcp::mcp_delete,
            commands::mcp::mcp_set_enabled,
            commands::mcp::mcp_reconnect,
            commands::mcp::mcp_set_permission,
            commands::mcp::mcp_import_preview,
            commands::mcp::mcp_import,
            commands::voice::audio_devices,
            commands::voice::voice_start,
            commands::voice::voice_stop,
            commands::voice::dictation_cancel,
            commands::voice::voice_status,
            commands::voice::tts_voices,
            commands::voice::tts_speak,
            commands::voice::tts_test,
            commands::voice::tts_stop,
            commands::voice::tts_set_paused,
            commands::voice::tts_state,
            commands::voice::tts_replay_last,
            commands::voice::models_catalog,
            commands::voice::models_installed,
            commands::voice::models_incompatible,
            commands::voice::models_download,
            commands::voice::models_cancel,
            commands::voice::models_delete,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Local Assistant")
        .run(|app, event| {
            // Keep running in the tray when all windows are closed.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
            let _ = app;
        });
}
