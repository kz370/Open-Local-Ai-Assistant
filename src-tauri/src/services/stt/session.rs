//! Microphone listening sessions: push-to-talk, hands-free (VAD) and dictation.

use super::SttService;
use crate::errors::{AppError, AppResult};
use crate::services::audio::capture::{Capture, CaptureEvent};
use crate::settings::SettingsStore;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ListenMode {
    /// Record until stopped, then transcribe (mic button / push-to-talk).
    PushToTalk,
    /// Continuous: VAD splits utterances, each is transcribed.
    HandsFree,
    /// Like push-to-talk, but the transcript is typed into the focused app.
    Dictation,
    /// Microphone test: level meter only.
    Test,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum VoiceEvent {
    /// "listening" | "transcribing" | "idle"
    State { mode: ListenMode, state: String, device: Option<String> },
    Level { mode: ListenMode, value: f32 },
    Transcript { mode: ListenMode, text: String, language: Option<String>, audio_ms: u64, elapsed_ms: u64 },
    Error { mode: ListenMode, code: String, detail: String },
}

pub type VoiceEmit = Arc<dyn Fn(VoiceEvent) + Send + Sync>;
/// Returns true while the assistant is speaking (hands-free ignores the mic then).
pub type SpeakingProbe = Arc<dyn Fn() -> bool + Send + Sync>;

struct Active {
    mode: ListenMode,
    stop: Arc<AtomicBool>,
    discard: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<()>,
}

pub struct VoiceSessions {
    stt: Arc<SttService>,
    settings: Arc<SettingsStore>,
    emit: VoiceEmit,
    speaking: SpeakingProbe,
    active: Mutex<Option<Active>>,
}

const MAX_RECORDING: Duration = Duration::from_secs(120);

impl VoiceSessions {
    pub fn new(stt: Arc<SttService>, settings: Arc<SettingsStore>, emit: VoiceEmit, speaking: SpeakingProbe) -> Self {
        Self { stt, settings, emit, speaking, active: Mutex::new(None) }
    }

    pub fn active_mode(&self) -> Option<ListenMode> {
        let mut guard = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if guard.as_ref().is_some_and(|a| a.thread.is_finished()) {
            *guard = None;
        }
        guard.as_ref().map(|a| a.mode)
    }

    pub fn start(&self, mode: ListenMode) -> AppResult<()> {
        let settings = self.settings.get();
        if mode != ListenMode::Test && !self.stt.is_ready(&settings.stt) {
            return Err(AppError::Stt("no local speech recognition model is installed".into()));
        }
        let vad_path = if mode == ListenMode::HandsFree {
            Some(self.stt.vad_model_path().ok_or_else(|| AppError::Stt("voice activity detection model is not installed".into()))?)
        } else {
            None
        };
        // Stop any running session first (discarding its audio).
        self.stop(true);

        let (capture, rx) = Capture::start(settings.stt.microphone.as_deref())?;
        let device = capture.device_name.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let discard = Arc::new(AtomicBool::new(false));
        let (stt, emit, speaking, store) = (self.stt.clone(), self.emit.clone(), self.speaking.clone(), self.settings.clone());
        let (stop2, discard2) = (stop.clone(), discard.clone());

        emit(VoiceEvent::State { mode, state: "listening".into(), device: Some(device) });

        let thread = std::thread::Builder::new()
            .name("voice-session".into())
            .spawn(move || {
                let mut capture = capture;
                let stt_settings = store.get().stt;
                let vad = vad_path.and_then(|p| {
                    let config = sherpa_onnx::VadModelConfig {
                        silero_vad: sherpa_onnx::SileroVadModelConfig {
                            model: Some(p.to_string_lossy().to_string()),
                            threshold: stt_settings.vad_threshold,
                            min_silence_duration: stt_settings.silence_ms as f32 / 1000.0,
                            min_speech_duration: 0.25,
                            window_size: 512,
                            max_speech_duration: 25.0,
                        },
                        sample_rate: 16_000,
                        num_threads: 1,
                        provider: Some("cpu".into()),
                        ..Default::default()
                    };
                    sherpa_onnx::VoiceActivityDetector::create(&config, 60.0)
                });
                if mode == ListenMode::HandsFree && vad.is_none() {
                    emit(VoiceEvent::Error { mode, code: "stt_unavailable".into(), detail: "failed to load voice activity detection".into() });
                    emit(VoiceEvent::State { mode, state: "idle".into(), device: None });
                    return;
                }

                let started = Instant::now();
                let mut buffer: Vec<f32> = Vec::new();
                let mut pending: Vec<f32> = Vec::new(); // VAD needs 512-sample windows
                let mut was_speaking = false;

                let transcribe = |samples: &[f32]| {
                    emit(VoiceEvent::State { mode, state: "transcribing".into(), device: None });
                    match stt.transcribe(samples, &stt_settings) {
                        Ok(t) => emit(VoiceEvent::Transcript { mode, text: t.text, language: t.language, audio_ms: t.audio_ms, elapsed_ms: t.elapsed_ms }),
                        Err(e) => {
                            tracing::warn!(error = %e, "transcription failed");
                            emit(VoiceEvent::Error { mode, code: e.code().into(), detail: e.to_string() });
                        }
                    }
                };

                loop {
                    if stop2.load(Ordering::Relaxed) {
                        break;
                    }
                    if mode != ListenMode::HandsFree && started.elapsed() > MAX_RECORDING {
                        break;
                    }
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(CaptureEvent::Level(v)) => emit(VoiceEvent::Level { mode, value: v }),
                        Ok(CaptureEvent::Samples(s)) => match (&vad, mode) {
                            (Some(vad), ListenMode::HandsFree) => {
                                let speaking_now = speaking();
                                if speaking_now {
                                    // Ignore our own voice from the speakers.
                                    pending.clear();
                                    if was_speaking {
                                        vad.reset();
                                    }
                                    was_speaking = true;
                                    continue;
                                }
                                if was_speaking {
                                    vad.reset();
                                    was_speaking = false;
                                }
                                pending.extend_from_slice(&s);
                                let mut offset = 0;
                                while pending.len() - offset >= 512 {
                                    vad.accept_waveform(&pending[offset..offset + 512]);
                                    offset += 512;
                                }
                                pending.drain(..offset);
                                while !vad.is_empty() {
                                    if let Some(seg) = vad.front() {
                                        let samples = seg.samples().to_vec();
                                        vad.pop();
                                        transcribe(&samples);
                                        emit(VoiceEvent::State { mode, state: "listening".into(), device: None });
                                    } else {
                                        vad.pop();
                                    }
                                }
                            }
                            (_, ListenMode::Test) => {}
                            _ => buffer.extend_from_slice(&s),
                        },
                        Ok(CaptureEvent::Error(e)) => {
                            emit(VoiceEvent::Error { mode, code: "audio".into(), detail: e });
                            break;
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
                capture.stop();
                // Drain samples captured just before stop.
                while let Ok(ev) = rx.try_recv() {
                    if let CaptureEvent::Samples(s) = ev {
                        if matches!(mode, ListenMode::PushToTalk | ListenMode::Dictation) {
                            buffer.extend_from_slice(&s);
                        }
                    }
                }
                if !discard2.load(Ordering::Relaxed) && matches!(mode, ListenMode::PushToTalk | ListenMode::Dictation) {
                    transcribe(&buffer);
                }
                emit(VoiceEvent::State { mode, state: "idle".into(), device: None });
            })
            .map_err(|e| AppError::Audio(e.to_string()))?;

        *self.active.lock().unwrap_or_else(|p| p.into_inner()) = Some(Active { mode, stop, discard, thread });
        Ok(())
    }

    /// Stops the active session. With `discard`, recorded audio is dropped
    /// instead of transcribed. Returns immediately; results arrive as events.
    pub fn stop(&self, discard: bool) -> Option<ListenMode> {
        let active = self.active.lock().unwrap_or_else(|p| p.into_inner()).take()?;
        active.discard.store(discard, Ordering::Relaxed);
        active.stop.store(true, Ordering::Relaxed);
        if discard {
            let _ = active.thread.join();
        }
        Some(active.mode)
    }
}
