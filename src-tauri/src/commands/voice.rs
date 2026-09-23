use super::CmdResult;
use crate::database::dictation::DictationEntry;
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

/// Stops listening and inserts what was heard so far (the overlay's "Insert now").
#[tauri::command]
pub fn dictation_insert_now(state: State<'_, AppState>) {
    if state.voice.active_mode() == Some(ListenMode::Dictation) {
        state.voice.stop(false);
    }
}

/// Inserts the (possibly edited) result the overlay holds in review mode.
#[tauri::command]
pub async fn dictation_confirm(app: AppHandle, text: String) -> CmdResult<()> {
    crate::confirm_review(app, text).await
}

/// Discards the current take (listening or under review) and listens again.
#[tauri::command]
pub async fn dictation_retry(app: AppHandle) {
    crate::retry_dictation(app).await;
}

/// Remembers the language chosen in the dictation overlay.
#[tauri::command]
pub fn dictation_set_language(app: AppHandle, state: State<'_, AppState>, language: String) -> CmdResult<()> {
    let saved = state.settings.update(|s| s.dictation.language = language)?;
    let _ = app.emit("settings://changed", &saved);
    Ok(())
}

#[tauri::command]
pub fn dictation_history(state: State<'_, AppState>) -> CmdResult<Vec<DictationEntry>> {
    state.db.dictation_list()
}

#[tauri::command]
pub fn dictation_history_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.db.dictation_delete(&id)
}

#[tauri::command]
pub fn dictation_history_clear(state: State<'_, AppState>) -> CmdResult<()> {
    state.db.dictation_clear()
}

/// Clears the dictation overlay's dragged position, reverting it to the
/// default centered-near-the-bottom placement.
#[tauri::command]
pub fn dictation_reset_overlay_position(app: AppHandle) {
    crate::desktop::window::reset_overlay_position(&app);
}

#[tauri::command]
pub fn voice_status(state: State<'_, AppState>) -> Option<ListenMode> {
    state.voice.active_mode()
}

/// Mutes or unmutes the microphone without ending the session, so a hands-free
/// call can be held without the assistant hearing the room.
#[tauri::command]
pub fn voice_set_muted(state: State<'_, AppState>, muted: bool) -> bool {
    state.voice.set_muted(muted);
    state.voice.is_muted()
}

#[tauri::command]
pub fn voice_muted(state: State<'_, AppState>) -> bool {
    state.voice.is_muted()
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
        Lang::Ar => "مَرْحَبًا! أَنَا مُسَاعِدُكَ الْمَحَلِّيُّ، وَكُلُّ مَا تَسْمَعُهُ صَوْتٌ مُوَلَّدٌ عَلَى هَذَا الْحَاسُوبِ.",
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


// ---------------------------------------------------------------------------
// GPU pack: optional CUDA libraries for the local speech models.
// ---------------------------------------------------------------------------

/// Key used for the GPU pack in the shared download table.
const GPU_DOWNLOAD: &str = "gpu-pack";

#[tauri::command]
pub fn gpu_status(state: State<'_, AppState>) -> crate::services::gpu::GpuStatus {
    let enabled = crate::services::gpu::is_enabled(&state.paths.data_dir);
    crate::services::gpu::status(&state.paths.data_dir, &state.hardware, enabled)
}

/// Turns GPU acceleration on or off. It takes effect after a restart, because
/// the libraries are chosen when the process starts.
#[tauri::command]
pub fn gpu_set_enabled(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    crate::services::gpu::set_enabled(&state.paths.data_dir, enabled)?;
    let _ = app.emit("gpu://changed", ());
    Ok(())
}

/// Downloads the CUDA pack. Called only after the user confirmed the download.
#[tauri::command]
pub async fn gpu_install(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    if !state.hardware.has_nvidia() {
        return Err(AppError::Invalid("no NVIDIA GPU was detected".into()));
    }
    let token = {
        let mut d = state.downloads.lock().unwrap_or_else(|p| p.into_inner());
        if d.contains_key(GPU_DOWNLOAD) {
            return Ok(()); // already running
        }
        d.entry(GPU_DOWNLOAD.to_string()).or_insert_with(CancellationToken::new).clone()
    };
    let data_dir = state.paths.data_dir.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let emitter = app2.clone();
        let progress = move |p: crate::services::gpu::GpuProgress| {
            let _ = emitter.emit("gpu://download", p);
        };
        let result = crate::services::gpu::install(&data_dir, token, &progress).await;
        let state = app2.state::<AppState>();
        state.downloads.lock().unwrap_or_else(|p| p.into_inner()).remove(GPU_DOWNLOAD);
        if result.is_ok() {
            // Installing it is what the user asked for, so switch it on too.
            let _ = crate::services::gpu::set_enabled(&state.paths.data_dir, true);
        }
        let _ = app2.emit("gpu://changed", ());
    });
    Ok(())
}

#[tauri::command]
pub fn gpu_cancel(state: State<'_, AppState>) {
    if let Some(t) = state.downloads.lock().unwrap_or_else(|p| p.into_inner()).get(GPU_DOWNLOAD) {
        t.cancel();
    }
}

#[tauri::command]
pub fn gpu_remove(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    crate::services::gpu::remove(&state.paths.data_dir)?;
    crate::services::gpu::set_enabled(&state.paths.data_dir, false)?;
    let _ = app.emit("gpu://changed", ());
    Ok(())
}

const SILMA_DOWNLOAD: &str = "__silma__";

#[tauri::command]
pub fn silma_status(state: State<'_, AppState>) -> crate::services::silma::SilmaStatus {
    state.silma.status()
}

/// Sets up SILMA (Python, PyTorch, weights). Called only after the user
/// confirmed the download.
#[tauri::command]
pub async fn silma_install(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let token = {
        let mut d = state.downloads.lock().unwrap_or_else(|p| p.into_inner());
        if d.contains_key(SILMA_DOWNLOAD) {
            return Ok(()); // already running
        }
        d.entry(SILMA_DOWNLOAD.to_string()).or_insert_with(CancellationToken::new).clone()
    };
    state.silma.stop();
    let (data_dir, nvidia) = (state.paths.data_dir.clone(), state.hardware.has_nvidia());
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let emitter = app2.clone();
        let progress = move |p: crate::services::silma::SilmaProgress| {
            let _ = emitter.emit("silma://progress", p);
        };
        let result = crate::services::silma::install(&data_dir, nvidia, token, &progress).await;
        let state = app2.state::<AppState>();
        state.downloads.lock().unwrap_or_else(|p| p.into_inner()).remove(SILMA_DOWNLOAD);
        if result.is_ok() {
            // Setting it up is what the user asked for: make it the Arabic voice.
            match state.settings.update(|s| {
                if let Some(e) = s.language.entries.iter_mut().find(|e| e.code == "ar") {
                    e.tts_voice = crate::services::tts::voices::SILMA_VOICE_ID.into();
                }
            }) {
                Ok(saved) => {
                    let _ = app2.emit("settings://changed", &saved);
                }
                Err(e) => tracing::warn!(error = %e, "could not select SILMA as the Arabic voice"),
            }
            if let Err(e) = state.silma.start() {
                tracing::warn!(error = %e, "SILMA could not start after setup");
            }
        }
        let _ = app2.emit("silma://status", state.silma.status());
    });
    Ok(())
}

/// Speaks a sample with SILMA itself, whichever Arabic voice is selected.
#[tauri::command]
pub async fn silma_test(state: State<'_, AppState>) -> CmdResult<()> {
    let (silma, tts) = (state.silma.clone(), state.tts.clone());
    let speed = state.settings.get().tts.speed;
    tokio::task::spawn_blocking(move || -> CmdResult<()> {
        let (samples, sample_rate) = silma.synthesize("مرحبًا! أنا مساعدك المحلي، وهذا صوتي الجديد باللغة العربية.", speed)?;
        tts.stop_all();
        tts.player.enqueue(crate::services::audio::playback::Clip { samples, sample_rate, tag: "test-voice".into() })?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Tts(e.to_string()))?
}

#[tauri::command]
pub fn silma_cancel(state: State<'_, AppState>) {
    if let Some(t) = state.downloads.lock().unwrap_or_else(|p| p.into_inner()).get(SILMA_DOWNLOAD) {
        t.cancel();
    }
}

#[tauri::command]
pub async fn silma_remove(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let silma = state.silma.clone();
    tokio::task::spawn_blocking(move || silma.remove()).await.map_err(|e| AppError::Tts(e.to_string()))??;
    let _ = app.emit("silma://status", state.silma.status());
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
