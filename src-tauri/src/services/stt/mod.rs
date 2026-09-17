//! Local speech-to-text with Whisper (sherpa-onnx). Audio never leaves the machine.

pub mod session;

use crate::errors::{AppError, AppResult};
use crate::services::hardware::HardwareInfo;
use crate::services::language::{detect, Lang};
use crate::services::models::catalog::{self, Engine, ModelKind};
use crate::services::models::{find_file, InstalledModel, ModelStore};
use crate::settings::SttSettings;
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
    recognizer: sherpa_onnx::OfflineRecognizer,
}

pub struct SttService {
    store: Arc<ModelStore>,
    hw: HardwareInfo,
    loaded: Mutex<Option<Loaded>>,
}

/// Minimum audio length worth transcribing (Whisper hallucinates on near-silence).
pub const MIN_AUDIO_MS: u64 = 350;

impl SttService {
    pub fn new(store: Arc<ModelStore>, hw: HardwareInfo) -> Self {
        Self { store, hw, loaded: Mutex::new(None) }
    }

    /// Chooses the configured model, or the best installed one automatically.
    pub fn resolve_model(&self, settings: &SttSettings) -> Option<InstalledModel> {
        let installed: Vec<InstalledModel> = self
            .store
            .installed()
            .into_iter()
            .filter(|m| m.kind == ModelKind::Stt && m.engine == Engine::Whisper)
            .collect();
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
                let quality = c.map(|c| c.quality).unwrap_or(3) as i64;
                let fits = c.map(|c| ram_gb >= c.min_ram_gb as u64).unwrap_or(true);
                (fits, quality)
            })
    }

    pub fn vad_model_path(&self) -> Option<std::path::PathBuf> {
        self.store
            .installed()
            .into_iter()
            .find(|m| m.kind == ModelKind::Vad)
            .and_then(|m| find_file(&m.path, |n| n.starts_with("silero_vad") && n.ends_with(".onnx")))
    }

    pub fn is_ready(&self, settings: &SttSettings) -> bool {
        self.resolve_model(settings).is_some()
    }

    /// Drops the loaded recognizer (e.g. after the model setting changed).
    pub fn unload(&self) {
        *self.loaded.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    fn create_recognizer(&self, model: &InstalledModel, language: &str) -> AppResult<sherpa_onnx::OfflineRecognizer> {
        let dir = &model.path;
        let enc = find_file(dir, |n| n.contains("encoder") && n.ends_with(".int8.onnx"))
            .or_else(|| find_file(dir, |n| n.contains("encoder") && n.ends_with(".onnx")))
            .ok_or_else(|| AppError::Stt("Whisper encoder file missing".into()))?;
        let dec = find_file(dir, |n| n.contains("decoder") && n.ends_with(".int8.onnx"))
            .or_else(|| find_file(dir, |n| n.contains("decoder") && n.ends_with(".onnx")))
            .ok_or_else(|| AppError::Stt("Whisper decoder file missing".into()))?;
        let tokens = find_file(dir, |n| n.ends_with("tokens.txt")).ok_or_else(|| AppError::Stt("tokens file missing".into()))?;
        let s = |p: std::path::PathBuf| Some(p.to_string_lossy().to_string());
        let mut config = sherpa_onnx::OfflineRecognizerConfig::default();
        config.model_config.whisper = sherpa_onnx::OfflineWhisperModelConfig {
            encoder: s(enc),
            decoder: s(dec),
            language: Some(language.to_string()),
            task: Some("transcribe".into()),
            tail_paddings: -1,
            ..Default::default()
        };
        config.model_config.tokens = s(tokens);
        config.model_config.num_threads = self.hw.inference_threads();
        config.model_config.provider = Some("cpu".into());
        config.decoding_method = Some("greedy_search".into());
        let started = Instant::now();
        let r = sherpa_onnx::OfflineRecognizer::create(&config).ok_or_else(|| AppError::Stt(format!("failed to load speech model {}", model.id)))?;
        tracing::info!(model = %model.id, language, ms = started.elapsed().as_millis() as u64, "stt model loaded");
        Ok(r)
    }

    /// Transcribes 16 kHz mono samples. Blocking: call from a worker thread.
    pub fn transcribe(&self, samples: &[f32], settings: &SttSettings) -> AppResult<Transcription> {
        let model = self
            .resolve_model(settings)
            .ok_or_else(|| AppError::Stt("no local speech recognition model is installed".into()))?;
        let audio_ms = samples.len() as u64 * 1000 / 16_000;
        if audio_ms < MIN_AUDIO_MS {
            return Ok(Transcription { text: String::new(), language: None, model_id: model.id, audio_ms, elapsed_ms: 0 });
        }
        // Forced language improves accuracy; "" lets Whisper detect it.
        let forced = Lang::from_code(&settings.language);
        let whisper_lang = forced.map(|l| l.code()).unwrap_or("");
        let key = format!("{}|{}", model.id, whisper_lang);

        let started = Instant::now();
        let mut guard = self.loaded.lock().unwrap_or_else(|p| p.into_inner());
        if guard.as_ref().map(|l| l.key != key).unwrap_or(true) {
            *guard = None; // free the previous model first
            *guard = Some(Loaded { key: key.clone(), recognizer: self.create_recognizer(&model, whisper_lang)? });
        }
        let recognizer = &guard.as_ref().expect("loaded").recognizer;
        // Whisper works on up to 30 s windows: decode long audio in chunks.
        let mut text = String::new();
        const CHUNK: usize = 16_000 * 28;
        for chunk in samples.chunks(CHUNK) {
            if chunk.len() < 16_000 * MIN_AUDIO_MS as usize / 1000 {
                continue;
            }
            let stream = recognizer.create_stream();
            stream.accept_waveform(16_000, chunk);
            recognizer.decode(&stream);
            let part = stream.get_result().map(|r| r.text).unwrap_or_default();
            let part = part.trim();
            if !part.is_empty() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(part);
            }
        }
        drop(guard);
        let text = clean_transcript(&text);
        let language = forced.or_else(|| detect(&text).map(|d| d.lang)).map(|l| l.code().to_string());
        Ok(Transcription { text, language, model_id: model.id, audio_ms, elapsed_ms: started.elapsed().as_millis() as u64 })
    }
}

/// Removes Whisper artifacts: bracketed non-speech tags and common
/// hallucinations on silence.
pub fn clean_transcript(text: &str) -> String {
    let mut t = text.trim().to_string();
    for tag in ["[BLANK_AUDIO]", "[MUSIC]", "(music)", "[Music]", "[silence]", "(silence)", "[NOISE]"] {
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
    fn cleans_whisper_artifacts() {
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
    }
}
