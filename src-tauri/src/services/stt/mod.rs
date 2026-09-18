//! Local speech-to-text. Audio never leaves the machine.
//!
//! Works with any compatible sherpa-onnx model folder (Whisper, NeMo/Parakeet
//! transducers, NeMo CTC, SenseVoice, Moonshine, Paraformer), including models
//! the user downloaded from Hugging Face into their own folders.

pub mod engine;
pub mod session;

use crate::errors::{AppError, AppResult};
use crate::services::hardware::HardwareInfo;
use crate::services::language::{detect, Lang};
use crate::services::models::catalog::{self, ModelKind};
use crate::services::models::{find_file, InstalledModel, ModelStore};
use crate::settings::SttSettings;
use engine::{EngineOptions, Recognizer};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Transcription {
    pub text: String,
    /// "en" | "ar" | "de" when recognized, otherwise None.
    pub language: Option<String>,
    pub model_id: String,
    pub audio_ms: u64,
    pub elapsed_ms: u64,
}

struct Loaded {
    key: String,
    recognizer: Recognizer,
}

pub struct SttService {
    store: Arc<ModelStore>,
    hw: HardwareInfo,
    loaded: Mutex<Option<Loaded>>,
}

/// Minimum audio length worth transcribing (models hallucinate on near-silence).
pub const MIN_AUDIO_MS: u64 = 350;

impl SttService {
    pub fn new(store: Arc<ModelStore>, hw: HardwareInfo) -> Self {
        Self { store, hw, loaded: Mutex::new(None) }
    }

    /// Chooses the configured model, or the best installed one automatically.
    pub fn resolve_model(&self, settings: &SttSettings) -> Option<InstalledModel> {
        let installed: Vec<InstalledModel> = self.store.installed().into_iter().filter(|m| m.kind == ModelKind::Stt).collect();
        if settings.model != "auto" {
            if let Some(m) = installed.iter().find(|m| m.id == settings.model) {
                return Some(m.clone());
            }
        }
        let ram_gb = self.hw.total_ram_bytes / (1024 * 1024 * 1024);
        installed
            .into_iter()
            .max_by_key(|m| {
                let c = catalog::find(&m.id);
                let quality = c.map(|c| c.quality as i64).unwrap_or(4); // user-supplied models rank above the tiny defaults
                let fits = c.map(|c| ram_gb >= c.min_ram_gb as u64).unwrap_or(true);
                (fits, quality)
            })
    }

    pub fn vad_model_path(&self) -> Option<std::path::PathBuf> {
        self.store
            .installed()
            .into_iter()
            .find(|m| m.kind == ModelKind::Vad)
            .and_then(|m| find_file(&m.path, |n| (n.starts_with("silero_vad") || n.starts_with("ten-vad")) && n.ends_with(".onnx")))
    }

    pub fn is_ready(&self, settings: &SttSettings) -> bool {
        self.resolve_model(settings).is_some()
    }

    /// True when the selected model produces live text while speaking.
    pub fn is_streaming(&self, settings: &SttSettings) -> bool {
        self.resolve_model(settings).map(|m| m.streaming).unwrap_or(false)
    }

    /// Drops the loaded recognizer (e.g. after the model setting changed).
    pub fn unload(&self) {
        *self.loaded.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    /// Recognizer options for `settings`. The language may be any Whisper
    /// language code; "auto" (or empty) lets the model detect it.
    pub fn options(&self, settings: &SttSettings) -> EngineOptions {
        EngineOptions {
            threads: self.hw.inference_threads(),
            language: match settings.language.as_str() {
                "auto" | "" => String::new(),
                code => code.to_string(),
            },
            endpoint_silence: settings.silence_ms as f32 / 1000.0,
            task: "transcribe".into(),
        }
    }

    /// Runs `f` with the loaded recognizer, loading it first if needed.
    /// The recognizer stays loaded for the next call.
    pub fn with_recognizer<R>(&self, settings: &SttSettings, f: impl FnOnce(&Recognizer, &InstalledModel) -> R) -> AppResult<R> {
        self.with_recognizer_opts(settings, self.options(settings), f)
    }

    /// Like [`Self::with_recognizer`], with caller-chosen recognizer options.
    pub fn with_recognizer_opts<R>(&self, settings: &SttSettings, opts: EngineOptions, f: impl FnOnce(&Recognizer, &InstalledModel) -> R) -> AppResult<R> {
        let model = self
            .resolve_model(settings)
            .ok_or_else(|| AppError::Stt("no local speech recognition model is installed".into()))?;
        let key = format!("{}|{}|{}|{}", model.path.display(), opts.language, opts.endpoint_silence, opts.task);
        let mut guard = self.loaded.lock().unwrap_or_else(|p| p.into_inner());
        if guard.as_ref().map(|l| l.key != key).unwrap_or(true) {
            *guard = None; // free the previous model before loading another
            let files = engine::detect(&model.path).ok_or_else(|| AppError::Stt(format!("{} is not a recognizable speech model folder", model.path.display())))?;
            let started = Instant::now();
            let recognizer = engine::create(&files, &opts)?;
            tracing::info!(model = %model.id, family = files.family.label(), ms = started.elapsed().as_millis() as u64, "speech model loaded");
            *guard = Some(Loaded { key, recognizer });
        }
        let loaded = guard.as_ref().expect("recognizer loaded");
        Ok(f(&loaded.recognizer, &model))
    }

    /// Transcribes 16 kHz mono samples. Blocking: call from a worker thread.
    pub fn transcribe(&self, samples: &[f32], settings: &SttSettings) -> AppResult<Transcription> {
        let audio_ms = samples.len() as u64 * 1000 / 16_000;
        if audio_ms < MIN_AUDIO_MS {
            let model_id = self.resolve_model(settings).map(|m| m.id).unwrap_or_default();
            return Ok(Transcription { text: String::new(), language: None, model_id, audio_ms, elapsed_ms: 0 });
        }
        let started = Instant::now();
        let (text, model_id) = self.with_recognizer(settings, |rec, model| {
            // Whisper works on 30 s windows: decode long audio in chunks.
            const CHUNK: usize = 16_000 * 28;
            let mut text = String::new();
            for chunk in samples.chunks(CHUNK) {
                if (chunk.len() as u64) * 1000 / 16_000 < MIN_AUDIO_MS {
                    continue;
                }
                let part = rec.transcribe(chunk);
                let part = part.trim();
                if !part.is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(part);
                }
            }
            (text, model.id.clone())
        })?;
        let text = clean_transcript(&text);
        let forced = Lang::from_code(&settings.language);
        let language = forced.or_else(|| detect(&text).map(|d| d.lang)).map(|l| l.code().to_string());
        Ok(Transcription { text, language, model_id, audio_ms, elapsed_ms: started.elapsed().as_millis() as u64 })
    }
}

/// Removes recognizer artifacts: bracketed non-speech tags and the common
/// hallucinations models produce on silence.
pub fn clean_transcript(text: &str) -> String {
    let mut t = text.trim().to_string();
    for tag in ["[BLANK_AUDIO]", "[MUSIC]", "(music)", "[Music]", "[silence]", "(silence)", "[NOISE]", "<|nospeech|>"] {
        t = t.replace(tag, "");
    }
    let lower = t.trim().to_lowercase();
    const HALLUCINATIONS: &[&str] = &[
        "thank you.", "thanks for watching!", "thank you for watching.", "you", ".", "untertitel der amara.org-community",
        "untertitelung des zdf, 2020", "ترجمة نانسي قنقر", "اشتركوا في القناة",
    ];
    if HALLUCINATIONS.contains(&lower.as_str()) {
        return String::new();
    }
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_artifacts() {
        assert_eq!(clean_transcript(" [BLANK_AUDIO] "), "");
        assert_eq!(clean_transcript("Thank you."), "");
        assert_eq!(clean_transcript("Hello   world [MUSIC]"), "Hello world");
        assert_eq!(clean_transcript("كيف حالك اليوم؟"), "كيف حالك اليوم؟");
    }

    #[test]
    fn no_model_installed_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let svc = SttService::new(Arc::new(ModelStore::new(dir.path().into())), HardwareInfo::default());
        let err = svc.transcribe(&vec![0.0; 16_000], &SttSettings::default()).unwrap_err();
        assert_eq!(err.code(), "stt_unavailable");
        assert!(!svc.is_ready(&SttSettings::default()));
        assert!(!svc.is_streaming(&SttSettings::default()));
    }

    #[test]
    fn user_models_outrank_the_tiny_default() {
        let app = tempfile::tempdir().unwrap();
        let store = Arc::new(ModelStore::new(app.path().to_path_buf()));
        // A catalog model (quality 1) plus a user-provided streaming model.
        let base = app.path().join("whisper-base");
        std::fs::create_dir_all(&base).unwrap();
        for f in ["encoder.int8.onnx", "decoder.int8.onnx", "tokens.txt", ".complete"] {
            std::fs::write(base.join(f), b"x").unwrap();
        }
        let custom = app.path().join("nemotron-3.5-asr-streaming-0.6b");
        std::fs::create_dir_all(&custom).unwrap();
        for f in ["encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"] {
            std::fs::write(custom.join(f), b"x").unwrap();
        }
        let svc = SttService::new(store, HardwareInfo { total_ram_bytes: 16 << 30, ..Default::default() });
        let mut settings = SttSettings::default();
        assert_eq!(svc.resolve_model(&settings).unwrap().id, "nemotron-3.5-asr-streaming-0.6b");
        assert!(svc.is_streaming(&settings));
        // Explicit choice wins.
        settings.model = "whisper-base".into();
        assert_eq!(svc.resolve_model(&settings).unwrap().id, "whisper-base");
        assert!(!svc.is_streaming(&settings));
    }
}
