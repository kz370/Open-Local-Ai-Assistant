use super::CmdResult;
use crate::errors::AppError;
use crate::services::audio::devices::{self, AudioDevice};
use crate::services::language::Lang;
use crate::services::models::catalog::{CatalogModel, CATALOG};
use crate::services::models::download;
use crate::services::stt::session::ListenMode;
use crate::services::tts::voices::VoiceInfo;
use crate::state::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevices {
    pub inputs: Vec<AudioDevice>,
    pub outputs: Vec<AudioDevice>,
}

#[tauri::command]
pub async fn audio_devices(state: State<'_, AppState>) -> CmdResult<AudioDevices> {
    let mic_only = state.settings.get().stt.mic_only;
    tokio::task::spawn_blocking(move || AudioDevices { inputs: devices::list_inputs(mic_only), outputs: devices::list_outputs() })
        .await
        .map_err(|e| AppError::Audio(e.to_string()))
}

#[tauri::command]
pub fn voice_start(state: State<'_, AppState>, mode: ListenMode) -> CmdResult<()> {
    if mode != ListenMode::Test {
        state.tts.stop_all();
    }
    state.voice.start(mode)
}

#[tauri::command]
pub fn voice_stop(state: State<'_, AppState>, discard: bool) -> Option<ListenMode> {
    state.voice.stop(discard)
}

/// Cancels in-progress dictation (Esc / overlay X): drops audio, skips the
/// insert, shows "cancelled" feedback. No-op when idle.
#[tauri::command]
pub fn dictation_cancel(app: AppHandle) {
    crate::desktop::window::cancel_dictation(&app);
}

#[tauri::command]
pub fn voice_status(state: State<'_, AppState>) -> Option<ListenMode> {
    state.voice.active_mode()
}

#[tauri::command]
pub fn tts_voices(state: State<'_, AppState>) -> Vec<VoiceInfo> {
    state.tts.voices()
}

#[tauri::command]
pub fn tts_speak(state: State<'_, AppState>, text: String, language: Option<String>, tag: Option<String>) -> CmdResult<()> {
    let lang = language.as_deref().and_then(Lang::from_code);
    if !state.tts.player.is_available() {
        return Err(AppError::Tts("audio output is not available".into()));
    }
    // Long answers are split into sentences and played one after another, so
    // speech starts immediately and can be paused or stopped at any point.
    let tag = tag.unwrap_or_else(|| format!("speak-{}", uuid::Uuid::new_v4().simple()));
    state.tts.speak(&tag, &text, lang);
    Ok(())
}

#[tauri::command]
pub fn tts_test(state: State<'_, AppState>, language: String) -> CmdResult<()> {
    let lang = Lang::from_code(&language).ok_or_else(|| AppError::Invalid("language".into()))?;
    if !state.tts.is_available(lang) {
        return Err(AppError::Tts(format!("no local voice installed for {}", lang.english_name())));
    }
    let text = match lang {
        Lang::En => "Hello! This is your local assistant speaking. Everything you hear was generated on this computer.",
        Lang::De => "Hallo! Hier spricht dein lokaler Assistent. Alles, was du hörst, wurde auf diesem Computer erzeugt.",
        Lang::Ar => "مرحباً! أنا مساعدك المحلي. كل ما تسمعه تم توليده على هذا الحاسوب.",
    };
    state.tts.speak("test-voice", text, Some(lang));
    Ok(())
}

#[tauri::command]
pub fn tts_stop(state: State<'_, AppState>) {
    state.tts.stop_all();
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsState {
    pub speaking: bool,
    pub paused: bool,
}

#[tauri::command]
pub fn tts_set_paused(state: State<'_, AppState>, paused: bool) -> TtsState {
    state.tts.set_paused(paused);
    TtsState { speaking: state.tts.has_audio(), paused: state.tts.is_paused() }
}

#[tauri::command]
pub fn tts_state(state: State<'_, AppState>) -> TtsState {
    TtsState { speaking: state.tts.has_audio(), paused: state.tts.is_paused() }
}

#[tauri::command]
pub fn tts_replay_last(state: State<'_, AppState>) -> bool {
    state.tts.replay_last()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    #[serde(flatten)]
    pub model: &'static CatalogModel,
    pub download_bytes: u64,
    pub installed: bool,
    pub recommended: bool,
    pub downloading: bool,
}

#[tauri::command]
pub fn models_catalog(state: State<'_, AppState>) -> Vec<CatalogEntry> {
    let rec = crate::services::models::recommend(&state.hardware).ids();
    let downloading = state.downloads.lock().unwrap_or_else(|p| p.into_inner());
    CATALOG
        .iter()
        .map(|m| CatalogEntry {
            model: m,
            download_bytes: m.download_size(),
            installed: state.models.is_installed(m.id),
            recommended: rec.contains(&m.id),
            downloading: downloading.contains_key(m.id),
        })
        .collect()
}

#[tauri::command]
pub fn models_installed(state: State<'_, AppState>) -> Vec<crate::services::models::InstalledModel> {
    state.models.installed()
}

/// Model folders the app found but cannot run (e.g. PyTorch-only exports).
#[tauri::command]
pub fn models_incompatible(state: State<'_, AppState>) -> Vec<crate::services::models::IncompatibleModel> {
    state.models.incompatible()
}

/// Downloads the given catalog models sequentially. Called only after the
/// user explicitly confirmed the download in the UI.
#[tauri::command]
pub async fn models_download(app: AppHandle, state: State<'_, AppState>, ids: Vec<String>) -> CmdResult<()> {
    let models: Vec<&'static CatalogModel> = ids.iter().filter_map(|id| crate::services::models::catalog::find(id)).collect();
    if models.len() != ids.len() {
        return Err(AppError::Invalid("unknown model id".into()));
    }
    let store = state.models.clone();
    let app2 = app.clone();
    // Reserve every requested model immediately: the UI greys them out as
    // "queued" straight away, so they cannot be selected twice.
    {
        let mut d = state.downloads.lock().unwrap_or_else(|p| p.into_inner());
        for m in &models {
            if !store.is_installed(m.id) {
                d.entry(m.id.to_string()).or_insert_with(CancellationToken::new);
            }
        }
    }
    let _ = app.emit("models://changed", ());
    tauri::async_runtime::spawn(async move {
        for m in models {
            let state = app2.state::<AppState>();
            if store.is_installed(m.id) {
                state.downloads.lock().unwrap_or_else(|p| p.into_inner()).remove(m.id);
                continue;
            }
            let token = {
                let d = state.downloads.lock().unwrap_or_else(|p| p.into_inner());
                match d.get(m.id) {
                    Some(t) => t.clone(),
                    None => continue, // cancelled before it started
                }
            };
            let emitter = app2.clone();
            let progress = move |p: download::DownloadProgress| {
                let _ = emitter.emit("models://download", p);
            };
            let result = download::install(&store, m, token, &progress).await;
            state.downloads.lock().unwrap_or_else(|p| p.into_inner()).remove(m.id);
            if result.is_ok() {
                state.stt.unload();
            }
        }
        let _ = app2.emit("models://changed", ());
    });
    Ok(())
}

#[tauri::command]
pub fn models_cancel(state: State<'_, AppState>, id: String) {
    if let Some(t) = state.downloads.lock().unwrap_or_else(|p| p.into_inner()).get(&id) {
        t.cancel();
    }
}

#[tauri::command]
pub fn models_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.stt.unload();
    state.models.delete(&id)?;
    let _ = app.emit("models://changed", ());
    Ok(())
}
