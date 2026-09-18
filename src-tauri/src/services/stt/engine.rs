//! Speech-recognition engine detection and creation.
//!
//! Supports the sherpa-onnx model families that people download from Hugging
//! Face: Whisper, offline/streaming transducers (NeMo Parakeet / Nemotron /
//! Zipformer), NeMo CTC, SenseVoice, Moonshine and Paraformer. Streaming
//! models also produce live partial text while you speak.

use crate::errors::{AppError, AppResult};
use crate::services::models::find_file;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SttFamily {
    Whisper,
    /// encoder + decoder + joiner, decoded in one pass.
    OfflineTransducer,
    /// encoder + decoder + joiner, streaming (live partial results).
    OnlineTransducer,
    NemoCtc,
    SenseVoice,
    Moonshine,
    Paraformer,
}

impl SttFamily {
    pub fn is_streaming(self) -> bool {
        self == SttFamily::OnlineTransducer
    }

    pub fn label(self) -> &'static str {
        match self {
            SttFamily::Whisper => "Whisper",
            SttFamily::OfflineTransducer => "Transducer",
            SttFamily::OnlineTransducer => "Streaming transducer",
            SttFamily::NemoCtc => "NeMo CTC",
            SttFamily::SenseVoice => "SenseVoice",
            SttFamily::Moonshine => "Moonshine",
            SttFamily::Paraformer => "Paraformer",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SttModelFiles {
    pub family: SttFamily,
    pub tokens: PathBuf,
    pub encoder: Option<PathBuf>,
    pub decoder: Option<PathBuf>,
    pub joiner: Option<PathBuf>,
    pub model: Option<PathBuf>,
    /// Moonshine only
    pub preprocessor: Option<PathBuf>,
    pub uncached_decoder: Option<PathBuf>,
    pub cached_decoder: Option<PathBuf>,
}

fn onnx_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(".onnx"))
                .collect()
        })
        .unwrap_or_default()
}

/// Prefers int8 quantized files (smaller, faster on CPU).
fn pick(dir: &Path, names: &[String], keyword: &str) -> Option<PathBuf> {
    let matching: Vec<&String> = names.iter().filter(|n| n.to_ascii_lowercase().contains(keyword)).collect();
    let best = matching
        .iter()
        .find(|n| n.contains(".int8."))
        .or_else(|| matching.iter().find(|n| !n.contains("fp16")))
        .or_else(|| matching.first())?;
    Some(dir.join(best.as_str()))
}

fn hints(dir: &Path) -> String {
    let mut s = dir.to_string_lossy().to_ascii_lowercase();
    if let Ok(readme) = std::fs::read_to_string(dir.join("README.md")) {
        s.push(' ');
        s.push_str(&readme.to_ascii_lowercase());
    }
    s
}

/// Inspects a folder and works out which kind of speech model it holds.
pub fn detect(dir: &Path) -> Option<SttModelFiles> {
    let names = onnx_files(dir);
    if names.is_empty() {
        return None;
    }
    let tokens = find_file(dir, |n| n.ends_with("tokens.txt"))?;
    let has = |kw: &str| names.iter().any(|n| n.to_ascii_lowercase().contains(kw));
    let text = hints(dir);
    let base = SttModelFiles {
        family: SttFamily::Whisper,
        tokens: tokens.clone(),
        encoder: None,
        decoder: None,
        joiner: None,
        model: None,
        preprocessor: None,
        uncached_decoder: None,
        cached_decoder: None,
    };

    if has("preprocess") && has("uncached_decode") {
        return Some(SttModelFiles {
            family: SttFamily::Moonshine,
            preprocessor: pick(dir, &names, "preprocess"),
            encoder: pick(dir, &names, "encode"),
            uncached_decoder: pick(dir, &names, "uncached_decode"),
            cached_decoder: pick(dir, &names, "cached_decode"),
            ..base
        });
    }
    if has("encoder") && has("decoder") && has("joiner") {
        // Streaming exports say so in their folder name or README.
        let streaming = ["streaming", "online", "stream", "chunk size", "chunk_size"].iter().any(|k| text.contains(k));
        return Some(SttModelFiles {
            family: if streaming { SttFamily::OnlineTransducer } else { SttFamily::OfflineTransducer },
            encoder: pick(dir, &names, "encoder"),
            decoder: pick(dir, &names, "decoder"),
            joiner: pick(dir, &names, "joiner"),
            ..base
        });
    }
    if has("encoder") && has("decoder") {
        return Some(SttModelFiles { family: SttFamily::Whisper, encoder: pick(dir, &names, "encoder"), decoder: pick(dir, &names, "decoder"), ..base });
    }
    // Single-file models: decide by name hints.
    let model = pick(dir, &names, ".onnx")?;
    let family = if text.contains("sense") {
        SttFamily::SenseVoice
    } else if text.contains("paraformer") {
        SttFamily::Paraformer
    } else if text.contains("ctc") || text.contains("nemo") || text.contains("parakeet") {
        SttFamily::NemoCtc
    } else {
        SttFamily::SenseVoice
    };
    Some(SttModelFiles { family, model: Some(model), ..base })
}

fn s(p: &Option<PathBuf>) -> Option<String> {
    p.as_ref().map(|p| p.to_string_lossy().to_string())
}

/// A loaded recognizer: offline models decode complete utterances, streaming
/// models also return partial text while audio is still arriving.
pub enum Recognizer {
    Offline(sherpa_onnx::OfflineRecognizer),
    Online(sherpa_onnx::OnlineRecognizer),
}

/// Live decoding session for a streaming recognizer.
pub struct StreamSession<'a> {
    recognizer: &'a sherpa_onnx::OnlineRecognizer,
    stream: sherpa_onnx::OnlineStream,
}

impl StreamSession<'_> {
    pub fn accept(&mut self, samples: &[f32]) {
        self.stream.accept_waveform(16_000, samples);
        while self.recognizer.is_ready(&self.stream) {
            self.recognizer.decode(&self.stream);
        }
    }

    pub fn text(&self) -> String {
        self.recognizer.get_result(&self.stream).map(|r| r.text).unwrap_or_default()
    }

    /// True when the speaker paused long enough to end an utterance.
    pub fn is_endpoint(&self) -> bool {
        self.recognizer.is_endpoint(&self.stream)
    }

    pub fn reset(&mut self) {
        self.recognizer.reset(&self.stream);
    }

    pub fn finish(&mut self) -> String {
        self.stream.input_finished();
        while self.recognizer.is_ready(&self.stream) {
            self.recognizer.decode(&self.stream);
        }
        self.text()
    }
}

impl Recognizer {
    pub fn is_streaming(&self) -> bool {
        matches!(self, Recognizer::Online(_))
    }

    /// Decodes a complete utterance (16 kHz mono).
    pub fn transcribe(&self, samples: &[f32]) -> String {
        match self {
            Recognizer::Offline(r) => {
                let stream = r.create_stream();
                stream.accept_waveform(16_000, samples);
                r.decode(&stream);
                stream.get_result().map(|x| x.text).unwrap_or_default()
            }
            Recognizer::Online(r) => {
                let stream = r.create_stream();
                stream.accept_waveform(16_000, samples);
                // Tail padding helps the encoder flush the last words.
                stream.accept_waveform(16_000, &vec![0.0; 8_000]);
                stream.input_finished();
                while r.is_ready(&stream) {
                    r.decode(&stream);
                }
                r.get_result(&stream).map(|x| x.text).unwrap_or_default()
            }
        }
    }

    pub fn stream_session(&self) -> Option<StreamSession<'_>> {
        match self {
            Recognizer::Online(r) => Some(StreamSession { recognizer: r, stream: r.create_stream() }),
            Recognizer::Offline(_) => None,
        }
    }
}

pub struct EngineOptions {
    pub threads: i32,
    /// Whisper only: "" lets the model detect the language.
    pub language: String,
    /// Streaming only: end an utterance after this much trailing silence.
    pub endpoint_silence: f32,
    /// Whisper only: "transcribe", or "translate" to English.
    pub task: String,
}

pub fn create(files: &SttModelFiles, opts: &EngineOptions) -> AppResult<Recognizer> {
    // Creating a recognizer proves the speech libraries loaded and ran.
    crate::services::gpu::mark_healthy();
    let tokens = Some(files.tokens.to_string_lossy().to_string());
    if files.family.is_streaming() {
        let mut config = sherpa_onnx::OnlineRecognizerConfig::default();
        config.model_config.transducer = sherpa_onnx::OnlineTransducerModelConfig {
            encoder: s(&files.encoder),
            decoder: s(&files.decoder),
            joiner: s(&files.joiner),
        };
        config.model_config.tokens = tokens;
        config.model_config.num_threads = opts.threads;
        config.model_config.provider = Some(crate::services::gpu::provider().into());
        config.decoding_method = Some("greedy_search".into());
        config.enable_endpoint = true;
        config.rule1_min_trailing_silence = 2.4;
        config.rule2_min_trailing_silence = opts.endpoint_silence.max(0.3);
        config.rule3_min_utterance_length = 20.0;
        return sherpa_onnx::OnlineRecognizer::create(&config)
            .map(Recognizer::Online)
            .ok_or_else(|| AppError::Stt("the streaming speech model could not be loaded".into()));
    }

    let mut config = sherpa_onnx::OfflineRecognizerConfig::default();
    config.model_config.tokens = tokens;
    config.model_config.num_threads = opts.threads;
    config.model_config.provider = Some(crate::services::gpu::provider().into());
    config.decoding_method = Some("greedy_search".into());
    match files.family {
        SttFamily::Whisper => {
            config.model_config.whisper = sherpa_onnx::OfflineWhisperModelConfig {
                encoder: s(&files.encoder),
                decoder: s(&files.decoder),
                language: Some(opts.language.clone()),
                task: Some(if opts.task == "translate" { "translate".into() } else { "transcribe".into() }),
                tail_paddings: -1,
                ..Default::default()
            };
        }
        SttFamily::OfflineTransducer => {
            config.model_config.transducer = sherpa_onnx::OfflineTransducerModelConfig {
                encoder: s(&files.encoder),
                decoder: s(&files.decoder),
                joiner: s(&files.joiner),
            };
        }
        SttFamily::NemoCtc => {
            config.model_config.nemo_ctc = sherpa_onnx::OfflineNemoEncDecCtcModelConfig { model: s(&files.model) };
        }
        SttFamily::SenseVoice => {
            config.model_config.sense_voice = sherpa_onnx::OfflineSenseVoiceModelConfig {
                model: s(&files.model),
                // SenseVoice knows only these; anything else is detected.
                language: Some(if ["zh", "en", "ja", "ko", "yue"].contains(&opts.language.as_str()) { opts.language.clone() } else { "auto".into() }),
                use_itn: true,
            };
        }
        SttFamily::Paraformer => {
            config.model_config.paraformer = sherpa_onnx::OfflineParaformerModelConfig { model: s(&files.model) };
        }
        SttFamily::Moonshine => {
            config.model_config.moonshine = sherpa_onnx::OfflineMoonshineModelConfig {
                preprocessor: s(&files.preprocessor),
                encoder: s(&files.encoder),
                uncached_decoder: s(&files.uncached_decoder),
                cached_decoder: s(&files.cached_decoder),
                ..Default::default()
            };
        }
        SttFamily::OnlineTransducer => unreachable!("handled above"),
    }
    if let Some(r) = sherpa_onnx::OfflineRecognizer::create(&config) {
        return Ok(Recognizer::Offline(r));
    }
    // A streaming export was mis-detected as offline: retry as streaming.
    if files.joiner.is_some() {
        let streaming = SttModelFiles { family: SttFamily::OnlineTransducer, ..files.clone() };
        if let Ok(r) = create(&streaming, opts) {
            tracing::info!("model loaded as a streaming transducer after the offline attempt failed");
            return Ok(r);
        }
    }
    Err(AppError::Stt("the speech model could not be loaded".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, names: &[&str]) {
        std::fs::create_dir_all(dir).unwrap();
        for n in names {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
    }

    #[test]
    fn detects_model_families() {
        let root = tempfile::tempdir().unwrap();

        let whisper = root.path().join("sherpa-onnx-whisper-small");
        write(&whisper, &["small-encoder.int8.onnx", "small-decoder.int8.onnx", "small-tokens.txt"]);
        let d = detect(&whisper).unwrap();
        assert_eq!(d.family, SttFamily::Whisper);
        assert!(d.encoder.unwrap().to_string_lossy().contains("encoder"));

        // The user's NVIDIA streaming model (README mentions streaming/chunk size).
        let stream = root.path().join("nemotron-3.5-asr-streaming-0.6b");
        write(&stream, &["encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"]);
        assert_eq!(detect(&stream).unwrap().family, SttFamily::OnlineTransducer);
        assert!(detect(&stream).unwrap().family.is_streaming());

        let offline_t = root.path().join("sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8");
        write(&offline_t, &["encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"]);
        assert_eq!(detect(&offline_t).unwrap().family, SttFamily::OfflineTransducer);

        let moonshine = root.path().join("sherpa-onnx-moonshine-tiny-en-int8");
        write(&moonshine, &["preprocess.onnx", "encode.int8.onnx", "uncached_decode.int8.onnx", "cached_decode.int8.onnx", "tokens.txt"]);
        assert_eq!(detect(&moonshine).unwrap().family, SttFamily::Moonshine);

        let sense = root.path().join("sherpa-onnx-sense-voice-zh-en-ja-ko-yue");
        write(&sense, &["model.int8.onnx", "tokens.txt"]);
        assert_eq!(detect(&sense).unwrap().family, SttFamily::SenseVoice);

        let ctc = root.path().join("sherpa-onnx-nemo-ctc-en-conformer-medium");
        write(&ctc, &["model.onnx", "tokens.txt"]);
        assert_eq!(detect(&ctc).unwrap().family, SttFamily::NemoCtc);

        // Not a speech model
        let junk = root.path().join("junk");
        write(&junk, &["readme.txt"]);
        assert!(detect(&junk).is_none());
    }

    #[test]
    fn prefers_int8_weights() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("whisper");
        write(&dir, &["encoder.onnx", "encoder.int8.onnx", "decoder.onnx", "decoder.int8.onnx", "tokens.txt"]);
        let d = detect(&dir).unwrap();
        assert!(d.encoder.unwrap().to_string_lossy().contains("int8"));
        assert!(d.decoder.unwrap().to_string_lossy().contains("int8"));
    }
}
