//! Live captions: subtitles for whatever the PC is playing (videos, calls,
//! streams), in any language the speech model knows.
//!
//! System audio is captured through WASAPI loopback, so the microphone is never
//! opened. Streaming models caption word by word. With Whisper-style models the
//! voice-activity detector cuts the audio into sentences; the sentence still
//! being spoken is re-decoded about once a second so captions keep up, and each
//! finished sentence becomes a final caption line.
//!
//! Captions run on their own recognizer, next to (never instead of) the chat
//! and dictation sessions.

use crate::errors::{AppError, AppResult};
use crate::services::audio::capture::{Capture, CaptureEvent};
use crate::services::stt::engine::Recognizer;
use crate::services::stt::{clean_transcript, SttService, MIN_AUDIO_MS};
use crate::settings::{Settings, SettingsStore, SttSettings};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CaptionEvent {
    /// "starting" | "listening" | "idle"
    State { state: String, device: Option<String> },
    /// The sentence currently being spoken (replaced as it grows).
    Partial { text: String },
    /// A finished caption line.
    Line { text: String },
    Error { code: String, detail: String },
}

pub type CaptionEmit = Arc<dyn Fn(CaptionEvent) + Send + Sync>;

struct Active {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<()>,
}

pub struct LiveCaptions {
    stt: Arc<SttService>,
    settings: Arc<SettingsStore>,
    emit: CaptionEmit,
    active: Mutex<Option<Active>>,
    /// The previous session's thread, joined by the next start (never by the UI).
    previous: Mutex<Option<std::thread::JoinHandle<()>>>,
}

const VAD_WINDOW: usize = 512;
/// Whisper decodes the running sentence at most this often.
const PARTIAL_EVERY: Duration = Duration::from_millis(900);
/// Long monologues are cut here so a caption line is finished regularly.
const MAX_SENTENCE_SECS: f32 = 8.0;

impl LiveCaptions {
    /// `stt` should be a service of its own: captions keep their recognizer
    /// loaded for as long as they run.
    pub fn new(stt: Arc<SttService>, settings: Arc<SettingsStore>, emit: CaptionEmit) -> Self {
        Self { stt, settings, emit, active: Mutex::new(None), previous: Mutex::new(None) }
    }

    pub fn is_active(&self) -> bool {
        let mut guard = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if guard.as_ref().is_some_and(|a| a.thread.is_finished()) {
            *guard = None;
        }
        guard.is_some()
    }

    /// Speech settings as seen by captions: their own model and language.
    fn stt_settings(settings: &Settings) -> SttSettings {
        let mut stt = settings.stt.clone();
        if settings.captions.model != "auto" {
            stt.model = settings.captions.model.clone();
        }
        stt.language = settings.captions.language.clone();
        stt
    }

    /// Starts captioning (restarting if already running, e.g. after the
    /// language changed).
    pub fn start(&self) -> AppResult<()> {
        let settings = self.settings.get();
        let stt_settings = Self::stt_settings(&settings);
        if !self.stt.is_ready(&stt_settings) {
            return Err(AppError::Stt("no local speech recognition model is installed".into()));
        }
        let streaming = self.stt.is_streaming(&stt_settings);
        let vad_path = if streaming { None } else { self.stt.vad_model_path() };
        if !streaming && vad_path.is_none() {
            return Err(AppError::Stt("live captions need the voice activity detection model".into()));
        }
        self.stop_and_wait();

        let (capture, rx) = Capture::start_loopback(settings.captions.audio_source.as_deref())?;
        let device = capture.device_name.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let mut opts = self.stt.options(&stt_settings);
        opts.task = if settings.captions.translate { "translate" } else { "transcribe" }.into();
        let (stt, emit, stop2) = (self.stt.clone(), self.emit.clone(), stop.clone());
        let captions = settings.captions.clone();

        emit(CaptionEvent::State { state: "starting".into(), device: Some(device.clone()) });
        let thread = std::thread::Builder::new()
            .name("live-captions".into())
            .spawn(move || {
                let mut capture = capture;
                let ctx = Ctx { emit: emit.clone(), stop: stop2 };
                let result = stt.with_recognizer_opts(&stt_settings, opts, |rec, model| {
                    tracing::info!(model = %model.id, streaming = rec.is_streaming(), language = %captions.language, translate = captions.translate, "live captions started");
                    emit(CaptionEvent::State { state: "listening".into(), device: Some(device) });
                    if rec.is_streaming() {
                        ctx.run_streaming(rec, &rx);
                    } else if let Some(vad) = vad_path.as_deref().and_then(|p| create_vad(p, &stt_settings)) {
                        ctx.run_buffered(rec, &rx, vad);
                    } else {
                        emit(CaptionEvent::Error { code: "stt_unavailable".into(), detail: "the voice activity detection model could not be loaded".into() });
                    }
                });
                if let Err(e) = result {
                    tracing::warn!(error = %e, "live captions could not start");
                    emit(CaptionEvent::Error { code: e.code().into(), detail: e.to_string() });
                }
                capture.stop();
                // Free the model: captions are usually off.
                stt.unload();
                emit(CaptionEvent::State { state: "idle".into(), device: None });
            })
            .map_err(|e| AppError::Audio(e.to_string()))?;
        *self.active.lock().unwrap_or_else(|p| p.into_inner()) = Some(Active { stop, thread });
        Ok(())
    }

    /// Stops captioning. Returns false when captions were not running.
    pub fn stop(&self) -> bool {
        let Some(active) = self.active.lock().unwrap_or_else(|p| p.into_inner()).take() else { return false };
        active.stop.store(true, Ordering::Relaxed);
        *self.previous.lock().unwrap_or_else(|p| p.into_inner()) = Some(active.thread);
        true
    }

    fn stop_and_wait(&self) {
        self.stop();
        if let Some(prev) = self.previous.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = prev.join();
        }
    }

    /// Settings that need a restart of a running caption session to apply.
    pub fn needs_restart(before: &Settings, after: &Settings) -> bool {
        let (a, b) = (&before.captions, &after.captions);
        a.language != b.language
            || a.translate != b.translate
            || a.audio_source != b.audio_source
            || Self::stt_settings(before).model != Self::stt_settings(after).model
            || before.stt.extra_model_dirs != after.stt.extra_model_dirs
    }
}

impl Drop for LiveCaptions {
    fn drop(&mut self) {
        self.stop();
    }
}

fn create_vad(path: &std::path::Path, settings: &SttSettings) -> Option<sherpa_onnx::VoiceActivityDetector> {
    let config = sherpa_onnx::VadModelConfig {
        silero_vad: sherpa_onnx::SileroVadModelConfig {
            model: Some(path.to_string_lossy().to_string()),
            threshold: settings.vad_threshold,
            // Short pauses end a caption line; speech on videos rarely pauses long.
            min_silence_duration: 0.35,
            min_speech_duration: 0.25,
            window_size: VAD_WINDOW as i32,
            max_speech_duration: MAX_SENTENCE_SECS,
        },
        sample_rate: 16_000,
        num_threads: 1,
        provider: Some(crate::services::gpu::provider().into()),
        ..Default::default()
    };
    sherpa_onnx::VoiceActivityDetector::create(&config, 60.0)
}

/// Transcript cleaned for display: also drops sound tags such as
/// "[Music]" or "(applause)", including ones a partial decode cut in half.
fn caption_text(text: &str) -> String {
    static TAGS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let tags = TAGS.get_or_init(|| regex::Regex::new(r"\[[^\]]*(\]|$)|\([^)]*(\)|$)|<\|[^|]*(\|>|$)").expect("valid regex"));
    clean_transcript(&tags.replace_all(text, " "))
}

struct Ctx {
    emit: CaptionEmit,
    stop: Arc<AtomicBool>,
}

impl Ctx {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// Loopback streams report the odd buffer under/overrun (a dropped
    /// packet, e.g. while the PC is busy): not worth stopping captions for.
    /// Anything else (device unplugged, ...) ends the session. Returns true
    /// when the error was fatal and has been reported.
    fn fatal(&self, detail: String) -> bool {
        let lower = detail.to_lowercase();
        if lower.contains("underrun") || lower.contains("overrun") {
            tracing::debug!(error = %detail, "caption audio glitch ignored");
            return false;
        }
        (self.emit)(CaptionEvent::Error { code: "audio".into(), detail });
        true
    }

    fn partial(&self, text: &str) {
        (self.emit)(CaptionEvent::Partial { text: caption_text(text) });
    }

    fn line(&self, text: &str) {
        let text = caption_text(text);
        if !text.is_empty() {
            (self.emit)(CaptionEvent::Line { text });
        }
    }

    fn run_streaming(&self, rec: &Recognizer, rx: &Receiver<CaptureEvent>) {
        let Some(mut session) = rec.stream_session() else { return };
        let mut last = String::new();
        while !self.stopped() {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(CaptureEvent::Samples(s)) => {
                    session.accept(&s);
                    let text = session.text().trim().to_string();
                    if session.is_endpoint() {
                        session.reset();
                        last.clear();
                        self.line(&text);
                    } else if text != last {
                        self.partial(&text);
                        last = text;
                    }
                }
                Ok(CaptureEvent::Level(_)) => {}
                Ok(CaptureEvent::Error(e)) => {
                    if self.fatal(e) {
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn run_buffered(&self, rec: &Recognizer, rx: &Receiver<CaptureEvent>, vad: sherpa_onnx::VoiceActivityDetector) {
        let mut pending: Vec<f32> = Vec::new(); // VAD needs fixed-size windows
        let mut live: Vec<f32> = Vec::new(); // the sentence still being spoken
        let mut next_partial = Instant::now();
        let mut shown_partial = false;
        let max_live = (MAX_SENTENCE_SECS * 16_000.0) as usize;
        let mut last_audio = Instant::now();

        while !self.stopped() {
            let samples = match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(CaptureEvent::Samples(s)) => {
                    last_audio = Instant::now();
                    s
                }
                Ok(CaptureEvent::Level(_)) => continue,
                Ok(CaptureEvent::Error(e)) => {
                    if self.fatal(e) {
                        return;
                    }
                    continue;
                }
                // Loopback delivers nothing at all while the PC is silent, so
                // the VAD would never hear the pause that ends a sentence:
                // feed it that silence.
                Err(RecvTimeoutError::Timeout) => {
                    let gap = last_audio.elapsed().min(Duration::from_secs(1));
                    last_audio = Instant::now();
                    vec![0.0; (gap.as_secs_f32() * 16_000.0) as usize]
                }
                Err(RecvTimeoutError::Disconnected) => return,
            };
            pending.extend_from_slice(&samples);
            let mut offset = 0;
            while pending.len() - offset >= VAD_WINDOW {
                vad.accept_waveform(&pending[offset..offset + VAD_WINDOW]);
                offset += VAD_WINDOW;
            }
            pending.drain(..offset);

            let mut finished = false;
            while !vad.is_empty() {
                let segment = vad.front().map(|s| s.samples().to_vec());
                vad.pop();
                let Some(segment) = segment else { continue };
                if (segment.len() as u64) * 1000 / 16_000 < MIN_AUDIO_MS {
                    continue;
                }
                self.line(&rec.transcribe(&segment));
                finished = true;
            }
            if finished {
                live.clear();
                shown_partial = false;
            }

            if vad.detected() {
                live.extend_from_slice(&samples);
                if live.len() > max_live {
                    live.drain(..live.len() - max_live);
                }
                let long_enough = (live.len() as u64) * 1000 / 16_000 >= MIN_AUDIO_MS;
                if long_enough && Instant::now() >= next_partial {
                    let t0 = Instant::now();
                    let text = caption_text(&rec.transcribe(&live));
                    if !text.is_empty() {
                        self.partial(&text);
                        shown_partial = true;
                    }
                    // A slow machine decodes less often instead of falling behind.
                    next_partial = Instant::now() + PARTIAL_EVERY.max(t0.elapsed() * 2);
                }
            } else {
                live.clear();
                if shown_partial && !finished {
                    // The VAD dropped the speech (too short / noise).
                    self.partial("");
                    shown_partial = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::caption_text;

    #[test]
    fn sound_tags_are_removed() {
        assert_eq!(caption_text("[BLANK"), "");
        assert_eq!(caption_text("(upbeat music) Hello there [Music]"), "Hello there");
        assert_eq!(caption_text("Guten Abend."), "Guten Abend.");
        assert_eq!(caption_text("مرحبا بكم"), "مرحبا بكم");
    }
}
