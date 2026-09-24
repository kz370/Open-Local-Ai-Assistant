//! "Models in memory": which models are loaded right now, and loading or
//! unloading them by hand. The same loaders warm everything up at startup
//! when "Load models when the app starts" is on.

use super::CmdResult;
use crate::errors::AppError;
use crate::services::language::Lang;
use crate::state::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    /// "stt" | "voice:en" | "voice:ar" | "voice:de" | "llm:<model id>"
    pub key: String,
    /// "stt" | "voice" | "llm"
    pub kind: String,
    /// What the model is used for (language code for voices).
    pub role: String,
    pub model: String,
    /// "loaded" | "loading" | "idle" | "missing" | "failed"
    pub state: String,
    pub detail: Option<String>,
    /// Key of this model's "load at startup" switch; empty when it has none.
    pub autoload_key: String,
    pub autoload: bool,
}

const LANGS: [Lang; 3] = [Lang::En, Lang::Ar, Lang::De];

fn lang_from_key(key: &str) -> Option<Lang> {
    key.strip_prefix("voice:").and_then(Lang::from_code)
}

fn changed(app: &AppHandle) {
    let _ = app.emit("memory://changed", ());
}

/// Marks `key` as loading while `f` runs.
async fn tracked<T>(app: &AppHandle, key: &str, f: impl std::future::Future<Output = CmdResult<T>>) -> CmdResult<T> {
    let state = app.state::<AppState>();
    state.model_loading.lock().unwrap_or_else(|p| p.into_inner()).insert(key.to_string());
    changed(app);
    let result = f.await;
    state.model_loading.lock().unwrap_or_else(|p| p.into_inner()).remove(key);
    changed(app);
    result
}

#[tauri::command]
pub async fn memory_status(state: State<'_, AppState>) -> CmdResult<Vec<MemoryItem>> {
    let loading = state.model_loading.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let state_of = |key: &str, loaded: bool| {
        if loading.contains(key) {
            "loading"
        } else if loaded {
            "loaded"
        } else {
            "idle"
        }
        .to_string()
    };
    let settings = state.settings.get();
    let mut items = Vec::new();
    // The ONNX speech models run wherever the GPU pack put this process.
    let onnx_device = Some(if crate::services::gpu::is_active() { "GPU" } else { "CPU" }.to_string());

    // Speech recognition
    match state.stt.resolve_model(&settings.stt) {
        Some(m) => items.push(MemoryItem {
            key: "stt".into(),
            kind: "stt".into(),
            role: "stt".into(),
            state: state_of("stt", state.stt.loaded_model().as_deref() == Some(m.id.as_str())),
            model: m.name,
            detail: onnx_device.clone(), ..Default::default() }),
        None => items.push(MemoryItem { key: "stt".into(), kind: "stt".into(), role: "stt".into(), model: String::new(), state: "missing".into(), detail: None, ..Default::default() }),
    }

    // One voice per language.
    let loaded_voices = state.tts.loaded_models();
    for lang in LANGS {
        let key = format!("voice:{}", lang.code());
        match state.tts.selected_voice(lang) {
            Some(v) => items.push(MemoryItem {
                state: state_of(&key, loaded_voices.contains(&v.model_id)),
                key,
                kind: "voice".into(),
                role: lang.code().into(),
                model: v.name,
                detail: onnx_device.clone(), ..Default::default() }),
            None => items.push(MemoryItem { key, kind: "voice".into(), role: lang.code().into(), model: String::new(), state: "missing".into(), detail: None, ..Default::default() }),
        }
    }

    // LM Studio: the chat model plus anything else it holds in memory. A hosted
    // provider keeps no models on this computer, so it has nothing to show here.
    if settings.ai.provider == "lmstudio" {
        match state.resolver.models(true).await {
            Ok(models) => {
                let chat_id = if settings.ai.model_mode == "manual" {
                    settings.ai.model.clone().filter(|m| !m.is_empty())
                } else {
                    state.resolver.auto_selection().await.ok().flatten().map(|s| s.model_id)
                };
                if let Some(id) = &chat_id {
                    let info = models.iter().find(|m| &m.id == id);
                    let key = format!("llm:{id}");
                    items.push(MemoryItem {
                        state: state_of(&key, info.is_some_and(|m| m.loaded)),
                        key,
                        kind: "llm".into(),
                        role: "chat".into(),
                        model: info.map(|m| m.display_name.clone()).unwrap_or_else(|| id.clone()),
                        detail: Some("LM Studio".into()), ..Default::default() });
                }
                for m in models.iter().filter(|m| m.loaded && Some(&m.id) != chat_id.as_ref()) {
                    let key = format!("llm:{}", m.id);
                    items.push(MemoryItem { state: state_of(&key, true), key, kind: "llm".into(), role: "other".into(), model: m.display_name.clone(), detail: Some("LM Studio".into()), ..Default::default() });
                }
            }
            Err(e) => items.push(MemoryItem {
                key: "llm".into(),
                kind: "llm".into(),
                role: "chat".into(),
                model: String::new(),
                state: "failed".into(),
                detail: Some(e.to_string()), ..Default::default() }),
        }
    }
    for item in &mut items {
        item.autoload_key = match (item.kind.as_str(), item.role.as_str()) {
            ("stt", _) | ("voice", _) => item.key.clone(),
            ("llm", "chat") => "llm".into(),
            _ => String::new(),
        };
        item.autoload = settings.general.autoloads(&item.autoload_key);
    }
    Ok(items)
}

/// Loads one model (see [`MemoryItem::key`]). Returns once it is in memory.
pub async fn load(app: &AppHandle, key: &str) -> CmdResult<()> {
    let state = app.state::<AppState>();
    match key {
        "stt" => {
            let (stt, settings) = (state.stt.clone(), state.settings.get().stt);
            tracked(app, key, async move {
                tokio::task::spawn_blocking(move || stt.with_recognizer(&settings, |_, _| ())).await.map_err(|e| AppError::Stt(e.to_string()))?
            })
            .await
        }
        k if k.starts_with("voice:") => {
            let lang = lang_from_key(k).ok_or_else(|| AppError::Invalid(k.into()))?;
            let (tts, threads) = (state.tts.clone(), state.hardware.inference_threads().min(4));
            tracked(app, key, async move {
                tokio::task::spawn_blocking(move || tts.preload_lang(lang, threads)).await.map_err(|e| AppError::Tts(e.to_string()))?
            })
            .await
        }
        // The chat model as a chat would pick it (auto or manual).
        "llm" => {
            let (resolver, ai) = (state.resolver.clone(), state.settings.get().ai);
            tracked(app, key, async move { resolver.resolve(&ai).await.map(|_| ()) }).await
        }
        k if k.starts_with("llm:") => {
            use crate::services::ai::AiService;
            let (lm, resolver, ctx) = (state.lmstudio.clone(), state.resolver.clone(), state.settings.get().ai.context_length);
            let id = k["llm:".len()..].to_string();
            tracked(app, key, async move {
                lm.load_model(&id, ctx).await?;
                resolver.invalidate().await;
                Ok(())
            })
            .await
        }
        _ => Err(AppError::Invalid(format!("unknown model {key}"))),
    }
}

/// Everything the assistant needs, in the background. `only_autoload` skips the
/// models switched off for startup (the "Load all" button loads them anyway).
pub fn load_all(app: &AppHandle, only_autoload: bool) {
    let state = app.state::<AppState>();
    let general = state.settings.get().general;
    let wanted = move |key: &str| !only_autoload || general.autoloads(key);
    let keys: Vec<&'static str> = ["stt", "voice:en", "voice:ar", "voice:de"].into_iter().filter(|k| wanted(k)).collect();
    let speech = app.clone();
    tauri::async_runtime::spawn(async move {
        let t0 = std::time::Instant::now();
        for key in keys {
            if let Err(e) = load(&speech, key).await {
                tracing::info!(model = key, error = %e, "not preloaded");
            }
        }
        tracing::info!(ms = t0.elapsed().as_millis() as u64, "speech and voice models loaded");
    });
    if wanted("llm") {
        let chat = app.clone();
        tauri::async_runtime::spawn(async move {
            // LM Studio may simply not be running yet; the first message retries.
            if let Err(e) = load(&chat, "llm").await {
                tracing::info!(error = %e, "chat model not preloaded");
            }
        });
    }
}

#[tauri::command]
pub async fn memory_load(app: AppHandle, key: String) -> CmdResult<()> {
    if key == "all" {
        load_all(&app, false);
        return Ok(());
    }
    load(&app, &key).await
}

#[tauri::command]
pub async fn memory_unload(app: AppHandle, state: State<'_, AppState>, key: String) -> CmdResult<()> {
    match key.as_str() {
        "stt" => {
            if state.voice.active_mode().is_some() {
                return Err(AppError::Invalid("speech recognition is in use; stop listening first".into()));
            }
            state.stt.unload();
        }
        k if k.starts_with("voice:") => {
            let lang = lang_from_key(k).ok_or_else(|| AppError::Invalid(k.into()))?;
            if let Some(v) = state.tts.selected_voice(lang) {
                state.tts.unload_model(&v.model_id);
            }
        }
        k if k.starts_with("llm:") => {
            state.lmstudio.unload_model(&k["llm:".len()..]).await?;
            state.resolver.invalidate().await;
        }
        _ => return Err(AppError::Invalid(format!("unknown model {key}"))),
    }
    changed(&app);
    Ok(())
}
