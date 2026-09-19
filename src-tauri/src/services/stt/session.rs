//! Microphone listening sessions: push-to-talk, hands-free (VAD) and dictation.
//!
//! Streaming models produce live partial text while you speak. With
//! non-streaming models (e.g. Whisper) the voice-activity detector splits your
//! speech into utterances, and each finished utterance is transcribed and shown
//! immediately, so dictation still updates as you talk.

use super::engine::Recognizer;
use super::SttService;
use crate::errors::{AppError, AppResult};
use crate::services::audio::capture::{Capture, CaptureEvent, LoopbackMonitor};
use crate::services::language::{detect, Lang};
use crate::settings::{SettingsStore, SttSettings};
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ListenMode {
    /// Record until stopped, then transcribe (mic button / push-to-talk).
    PushToTalk,
    /// Continuous: utterances are transcribed as they finish.
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
    State { mode: ListenMode, state: String, device: Option<String>, streaming: bool },
    /// `bands` is the voice spectrum (see `audio::spectrum`), all zero when muted.
    Level { mode: ListenMode, value: f32, bands: Vec<f32> },
    /// Live text while speaking (not final).
    Partial { mode: ListenMode, text: String },
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
    /// Set while the user has muted the microphone from the call screen. The
    /// session keeps running, it just ignores everything it hears.
    muted: Arc<AtomicBool>,
    active: Mutex<Option<Active>>,
    /// The previous session's thread, joined by the next session (never by the UI).
    previous: Mutex<Option<std::thread::JoinHandle<()>>>,
}

const MAX_RECORDING: Duration = Duration::from_secs(300);
const VAD_WINDOW: usize = 512;

impl VoiceSessions {
    pub fn new(stt: Arc<SttService>, settings: Arc<SettingsStore>, emit: VoiceEmit, speaking: SpeakingProbe) -> Self {
        Self { stt, settings, emit, speaking, muted: Arc::new(AtomicBool::new(false)), active: Mutex::new(None), previous: Mutex::new(None) }
    }

    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub fn active_mode(&self) -> Option<ListenMode> {
        let mut guard = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if guard.as_ref().is_some_and(|a| a.thread.is_finished()) {
            *guard = None;
        }
        guard.as_ref().map(|a| a.mode)
    }

    pub fn start(&self, mode: ListenMode) -> AppResult<()> {
        let mut settings = self.settings.get();
        // Resolve the language-entry's STT hint (usually == code, but lets a
        // user-added language differ) at this single choke point, which every
        // session-start path (push-to-talk, hands-free, dictation, test) goes
        // through — nothing downstream needs to know about `entries`.
        let stt_lang = settings.language.entries.iter().find(|e| e.code == settings.stt.language).map(|e| e.stt_language.clone());
        if let Some(l) = stt_lang {
            settings.stt.language = l;
        }
        if mode != ListenMode::Test && !self.stt.is_ready(&settings.stt) {
            return Err(AppError::Stt("no local speech recognition model is installed".into()));
        }
        let streaming = mode != ListenMode::Test && self.stt.is_streaming(&settings.stt);
        let vad_path = if mode != ListenMode::Test && !streaming { self.stt.vad_model_path() } else { None };
        if mode == ListenMode::HandsFree && !streaming && vad_path.is_none() {
            return Err(AppError::Stt("hands-free needs the voice activity detection model".into()));
        }
        // Stop any running session first (discarding its audio) and wait for it
        // here, before opening the microphone again.
        self.stop(true);
        if let Some(prev) = self.previous.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = prev.join();
        }

        let (capture, rx) = Capture::start(settings.stt.microphone.as_deref(), settings.stt.mic_only)?;
        let device = capture.device_name.clone();
        // Best-effort: if the speaker loopback can't be opened, sessions just
        // run without system-audio gating instead of failing to start.
        let loopback = if settings.stt.isolate_system_audio && mode != ListenMode::Test {
            // Watch the very output the assistant speaks through, not just the
            // system default, or nothing is gated when they differ.
            match LoopbackMonitor::start(settings.tts.output_device.as_deref()) {
                Ok(m) => Some(m),
                Err(e) => {
                    tracing::warn!(error = %e, "system audio isolation is on but the speaker monitor could not start");
                    None
                }
            }
        } else {
            None
        };
        let system_audio = loopback.as_ref().map(|l| l.level_handle());
        let stop = Arc::new(AtomicBool::new(false));
        let discard = Arc::new(AtomicBool::new(false));
        // A fresh session always starts listening: a mute belongs to the call
        // the user muted, not to the next one.
        self.muted.store(false, Ordering::Relaxed);
        let (stt, emit, speaking, muted) = (self.stt.clone(), self.emit.clone(), self.speaking.clone(), self.muted.clone());
        let stt_settings = settings.stt.clone();
        let (stop2, discard2) = (stop.clone(), discard.clone());

        emit(VoiceEvent::State { mode, state: "listening".into(), device: Some(device), streaming });

        let thread = std::thread::Builder::new()
            .name("voice-session".into())
            .spawn(move || {
                let (mut capture, mut loopback) = (capture, loopback);
                let ctx = SessionCtx {
                    mode,
                    emit: emit.clone(),
                    speaking,
                    muted,
                    settings: stt_settings.clone(),
                    stop: stop2,
                    discard: discard2,
                    system_audio,
                    muted_until: Cell::new(None),
                };
                run_session(&stt, &ctx, &rx, vad_path.as_deref());
                capture.stop();
                if let Some(lb) = loopback.as_mut() {
                    lb.stop();
                }
                emit(VoiceEvent::State { mode, state: "idle".into(), device: None, streaming });
            })
            .map_err(|e| AppError::Audio(e.to_string()))?;

        *self.active.lock().unwrap_or_else(|p| p.into_inner()) = Some(Active { mode, stop, discard, thread });
        Ok(())
    }

    /// Stops the active session. With `discard`, recorded audio is dropped
    /// instead of transcribed. Never blocks: the session thread finishes on its
    /// own and results arrive as events.
    pub fn stop(&self, discard: bool) -> Option<ListenMode> {
        let active = self.active.lock().unwrap_or_else(|p| p.into_inner()).take()?;
        active.discard.store(discard, Ordering::Relaxed);
        active.stop.store(true, Ordering::Relaxed);
        *self.previous.lock().unwrap_or_else(|p| p.into_inner()) = Some(active.thread);
        Some(active.mode)
    }
}

/// Runs one listening session over an audio source. Shared by the microphone
/// path and by tests that feed recorded audio.
fn run_session(stt: &SttService, ctx: &SessionCtx, rx: &Receiver<CaptureEvent>, vad_path: Option<&std::path::Path>) {
    if ctx.mode == ListenMode::Test {
        ctx.run_level_only(rx);
        return;
    }
    let vad = match vad_path {
        Some(p) => match create_vad(p, &ctx.settings) {
            Some(v) => Some(v),
            None => {
                tracing::warn!(path = %p.display(), "voice activity detection model could not be loaded");
                (ctx.emit)(VoiceEvent::Error { mode: ctx.mode, code: "stt_unavailable".into(), detail: "the voice activity detection model could not be loaded".into() });
                return;
            }
        },
        None => None,
    };
    let result = stt.with_recognizer(&ctx.settings, |rec, model| {
        tracing::info!(mode = ?ctx.mode, model = %model.id, streaming = rec.is_streaming(), vad = vad.is_some(), "listening session started");
        ctx.run(rec, rx, vad)
    });
    if let Err(e) = result {
        tracing::warn!(error = %e, "speech session could not start");
        (ctx.emit)(VoiceEvent::Error { mode: ctx.mode, code: e.code().into(), detail: e.to_string() });
    }
}

/// Test hook: runs a session over a caller-provided audio source (no microphone).
pub fn run_session_for_test(
    stt: &SttService,
    settings: &SttSettings,
    mode: ListenMode,
    rx: &Receiver<CaptureEvent>,
    vad_path: Option<&std::path::Path>,
    emit: VoiceEmit,
    stop: Arc<AtomicBool>,
) {
    let ctx = SessionCtx {
        mode,
        emit,
        speaking: Arc::new(|| false),
        muted: Arc::new(AtomicBool::new(false)),
        settings: settings.clone(),
        stop,
        discard: Arc::new(AtomicBool::new(false)),
        system_audio: None,
        muted_until: Cell::new(None),
    };
    run_session(stt, &ctx, rx, vad_path);
}

fn create_vad(path: &std::path::Path, settings: &SttSettings) -> Option<sherpa_onnx::VoiceActivityDetector> {
    let config = sherpa_onnx::VadModelConfig {
        silero_vad: sherpa_onnx::SileroVadModelConfig {
            model: Some(path.to_string_lossy().to_string()),
            threshold: settings.vad_threshold,
            min_silence_duration: settings.silence_ms as f32 / 1000.0,
            min_speech_duration: 0.25,
            window_size: VAD_WINDOW as i32,
            max_speech_duration: 20.0,
        },
        sample_rate: 16_000,
        num_threads: 1,
        provider: Some(crate::services::gpu::provider().into()),
        ..Default::default()
    };
    sherpa_onnx::VoiceActivityDetector::create(&config, 60.0)
}

struct SessionCtx {
    mode: ListenMode,
    emit: VoiceEmit,
    speaking: SpeakingProbe,
    /// Mirrors [`VoiceSessions::set_muted`].
    muted: Arc<AtomicBool>,
    settings: SttSettings,
    stop: Arc<AtomicBool>,
    discard: Arc<AtomicBool>,
    /// Live speaker-loopback level, when "isolate system audio" is on.
    system_audio: Option<Arc<AtomicU32>>,
    /// Keeps the microphone gated for a short tail after the speakers go quiet.
    /// Captured audio reaches this loop later than the loopback level does, so
    /// without the tail the end of every spoken phrase still bleeds in.
    muted_until: Cell<Option<Instant>>,
}

/// Watches how long the microphone has heard nothing and ends the session
/// when the configured limit is reached.
struct SilenceGuard {
    last_speech: Instant,
    limit: Option<Duration>,
}

impl SilenceGuard {
    fn new(mode: ListenMode, settings: &SttSettings) -> Self {
        let secs = if mode == ListenMode::HandsFree { settings.hands_free_timeout_secs } else { settings.auto_stop_silence_secs };
        Self { last_speech: Instant::now(), limit: (secs > 0).then(|| Duration::from_secs(secs as u64)) }
    }

    fn heard_speech(&mut self) {
        self.last_speech = Instant::now();
    }

    fn expired(&self) -> bool {
        self.limit.map(|l| self.last_speech.elapsed() > l).unwrap_or(false)
    }
}

/// Level above which we consider the microphone to be picking up speech.
const SPEECH_LEVEL: f32 = 0.22;
/// Level above which we consider the speakers to be audibly playing something.
const SYSTEM_AUDIO_LEVEL: f32 = 0.12;
/// How long the microphone stays gated after the last audible speaker sample:
/// covers the playback/capture latency and the room tail of the last word, and
/// bridges the short silences between words so the gate does not flap.
const AUDIO_TAIL: Duration = Duration::from_millis(700);

impl SessionCtx {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// True while this sample should be ignored instead of transcribed: the
    /// user muted the microphone, or the assistant is speaking (hands-free), or
    /// "isolate system audio" is on and the speakers are audibly playing
    /// something, or either was true within the last `AUDIO_TAIL`.
    /// True while the user has muted the microphone from the call screen.
    fn mic_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    fn input_muted(&self, hands_free: bool) -> bool {
        if self.mic_muted() {
            return true;
        }
        let assistant = hands_free && (self.speaking)();
        let speakers = self.system_audio.as_ref().is_some_and(|a| f32::from_bits(a.load(Ordering::Relaxed)) > SYSTEM_AUDIO_LEVEL);
        if assistant || speakers {
            self.muted_until.set(Some(Instant::now() + AUDIO_TAIL));
            return true;
        }
        match self.muted_until.get() {
            Some(t) if Instant::now() < t => true,
            Some(_) => {
                self.muted_until.set(None);
                false
            }
            None => false,
        }
    }

    fn emit_state(&self, state: &str) {
        (self.emit)(VoiceEvent::State { mode: self.mode, state: state.into(), device: None, streaming: false });
    }

    fn emit_partial(&self, text: &str) {
        (self.emit)(VoiceEvent::Partial { mode: self.mode, text: super::clean_transcript(text) });
    }

    fn emit_transcript(&self, text: &str, audio_ms: u64, elapsed_ms: u64) {
        let text = super::clean_transcript(text);
        let language = Lang::from_code(&self.settings.language)
            .or_else(|| detect(&text).map(|d| d.lang))
            .map(|l| l.code().to_string());
        (self.emit)(VoiceEvent::Transcript { mode: self.mode, text, language, audio_ms, elapsed_ms });
    }

    fn run_level_only(&self, rx: &Receiver<CaptureEvent>) {
        while !self.stopped() {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(CaptureEvent::Level(v, bands)) => {
                    let muted = self.mic_muted();
                    (self.emit)(VoiceEvent::Level { mode: self.mode, value: if muted { 0.0 } else { v }, bands: if muted { vec![0.0; bands.len()] } else { bands } })
                }
                Ok(CaptureEvent::Error(e)) => {
                    (self.emit)(VoiceEvent::Error { mode: self.mode, code: "audio".into(), detail: e });
                    return;
                }
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn run(&self, rec: &Recognizer, rx: &Receiver<CaptureEvent>, vad: Option<sherpa_onnx::VoiceActivityDetector>) {
        if rec.is_streaming() {
            self.run_streaming(rec, rx);
        } else {
            self.run_buffered(rec, rx, vad);
        }
    }

    /// Streaming recognizers: text appears while the user is still speaking.
    fn run_streaming(&self, rec: &Recognizer, rx: &Receiver<CaptureEvent>) {
        let Some(mut session) = rec.stream_session() else { return };
        let started = Instant::now();
        let mut last_partial = String::new();
        let mut committed = String::new();
        let mut samples_seen = 0u64;
        let hands_free = self.mode == ListenMode::HandsFree;
        let mut silence = SilenceGuard::new(self.mode, &self.settings);

        while !self.stopped() && started.elapsed() < MAX_RECORDING && !silence.expired() {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(CaptureEvent::Level(v, bands)) => {
                    // A muted microphone shows no level, and its silence must
                    // not count toward the idle timeout: the user is still here.
                    let muted = self.mic_muted();
                    if muted || v > SPEECH_LEVEL {
                        silence.heard_speech();
                    }
                    (self.emit)(VoiceEvent::Level { mode: self.mode, value: if muted { 0.0 } else { v }, bands: if muted { vec![0.0; bands.len()] } else { bands } });
                }
                Ok(CaptureEvent::Samples(s)) => {
                    if self.input_muted(hands_free) {
                        silence.heard_speech(); // the assistant/PC is making noise, not a silent room
                        continue; // ignore audio bleeding in from the speakers
                    }
                    samples_seen += s.len() as u64;
                    session.accept(&s);
                    let text = session.text();
                    if text != last_partial {
                        silence.heard_speech();
                        last_partial = text.clone();
                        let shown = if committed.is_empty() { text.clone() } else { format!("{committed} {text}") };
                        self.emit_partial(shown.trim());
                    }
                    if session.is_endpoint() {
                        let utterance = session.text();
                        session.reset();
                        last_partial.clear();
                        let utterance = utterance.trim().to_string();
                        if utterance.is_empty() {
                            continue;
                        }
                        if hands_free {
                            self.emit_transcript(&utterance, samples_seen * 1000 / 16_000, started.elapsed().as_millis() as u64);
                            samples_seen = 0;
                        } else {
                            if !committed.is_empty() {
                                committed.push(' ');
                            }
                            committed.push_str(&utterance);
                            self.emit_partial(&committed);
                        }
                    }
                }
                Ok(CaptureEvent::Error(e)) => {
                    (self.emit)(VoiceEvent::Error { mode: self.mode, code: "audio".into(), detail: e });
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        // Drain whatever was captured just before the stop.
        while let Ok(CaptureEvent::Samples(s)) = rx.try_recv() {
            samples_seen += s.len() as u64;
            session.accept(&s);
        }
        if hands_free || self.discard.load(Ordering::Relaxed) {
            return;
        }
        self.emit_state("transcribing");
        let tail = session.finish();
        let mut text = committed;
        let tail = tail.trim();
        if !tail.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(tail);
        }
        self.emit_transcript(&text, samples_seen * 1000 / 16_000, 0);
    }

    /// Non-streaming recognizers: the VAD splits speech into utterances.
    fn run_buffered(&self, rec: &Recognizer, rx: &Receiver<CaptureEvent>, vad: Option<sherpa_onnx::VoiceActivityDetector>) {
        let started = Instant::now();
        let hands_free = self.mode == ListenMode::HandsFree;
        let mut buffer: Vec<f32> = Vec::new(); // audio not yet covered by a finished utterance
        let mut pending: Vec<f32> = Vec::new(); // VAD needs fixed-size windows
        let mut committed = String::new();
        let mut was_speaking = false;
        let mut samples_seen = 0u64;
        let mut silence = SilenceGuard::new(self.mode, &self.settings);

        while !self.stopped() && (hands_free || started.elapsed() < MAX_RECORDING) && !silence.expired() {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(CaptureEvent::Level(v, bands)) => {
                    // A muted microphone shows no level, and its silence must
                    // not count toward the idle timeout: the user is still here.
                    let muted = self.mic_muted();
                    if muted || v > SPEECH_LEVEL {
                        silence.heard_speech();
                    }
                    (self.emit)(VoiceEvent::Level { mode: self.mode, value: if muted { 0.0 } else { v }, bands: if muted { vec![0.0; bands.len()] } else { bands } });
                }
                Ok(CaptureEvent::Samples(s)) => {
                    if self.input_muted(hands_free) {
                        silence.heard_speech();
                        pending.clear();
                        if let (Some(vad), false) = (vad.as_ref(), was_speaking) {
                            vad.reset();
                        }
                        was_speaking = true;
                        continue;
                    }
                    if was_speaking {
                        if let Some(vad) = vad.as_ref() {
                            vad.reset();
                        }
                        was_speaking = false;
                    }
                    samples_seen += s.len() as u64;
                    if !hands_free {
                        buffer.extend_from_slice(&s);
                    }
                    let Some(vad) = vad.as_ref() else { continue };
                    pending.extend_from_slice(&s);
                    let mut offset = 0;
                    while pending.len() - offset >= VAD_WINDOW {
                        vad.accept_waveform(&pending[offset..offset + VAD_WINDOW]);
                        offset += VAD_WINDOW;
                    }
                    pending.drain(..offset);
                    while !vad.is_empty() {
                        let segment = vad.front().map(|s| s.samples().to_vec());
                        vad.pop();
                        let Some(segment) = segment else { continue };
                        let seg_ms = segment.len() as u64 * 1000 / 16_000;
                        if seg_ms < super::MIN_AUDIO_MS {
                            continue;
                        }
                        let t0 = Instant::now();
                        let text = super::clean_transcript(&rec.transcribe(&segment));
                        if text.is_empty() {
                            continue;
                        }
                        silence.heard_speech();
                        if hands_free {
                            tracing::info!(chars = text.chars().count(), audio_ms = seg_ms, "utterance transcribed");
                            self.emit_transcript(&text, seg_ms, t0.elapsed().as_millis() as u64);
                            self.emit_state("listening");
                        } else {
                            // Utterance finished: show it live and drop its audio
                            // from the buffer so it is not transcribed twice.
                            if !committed.is_empty() {
                                committed.push(' ');
                            }
                            committed.push_str(&text);
                            self.emit_partial(&committed);
                            let consumed = (segment.len() + 16_000 / 2).min(buffer.len());
                            buffer.drain(..consumed);
                        }
                    }
                }
                Ok(CaptureEvent::Error(e)) => {
                    (self.emit)(VoiceEvent::Error { mode: self.mode, code: "audio".into(), detail: e });
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        while let Ok(CaptureEvent::Samples(s)) = rx.try_recv() {
            samples_seen += s.len() as u64;
            if !hands_free {
                buffer.extend_from_slice(&s);
            }
        }
        if hands_free || self.discard.load(Ordering::Relaxed) {
            return;
        }
        self.emit_state("transcribing");
        let t0 = Instant::now();
        let mut text = committed;
        if buffer.len() as u64 * 1000 / 16_000 >= super::MIN_AUDIO_MS {
            let tail = super::clean_transcript(&rec.transcribe(&buffer));
            if !tail.is_empty() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&tail);
            }
        }
        self.emit_transcript(&text, samples_seen * 1000 / 16_000, t0.elapsed().as_millis() as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(system_audio: Option<Arc<AtomicU32>>, speaking: bool) -> SessionCtx {
        SessionCtx {
            mode: ListenMode::Dictation,
            emit: Arc::new(|_| {}),
            speaking: Arc::new(move || speaking),
            muted: Arc::new(AtomicBool::new(false)),
            settings: SttSettings::default(),
            stop: Arc::new(AtomicBool::new(false)),
            discard: Arc::new(AtomicBool::new(false)),
            system_audio,
            muted_until: Cell::new(None),
        }
    }

    #[test]
    fn no_gating_without_the_monitor() {
        assert!(!ctx(None, false).input_muted(false));
    }

    #[test]
    fn muting_gates_the_microphone_until_it_is_unmuted() {
        let c = ctx(None, false);
        c.muted.store(true, Ordering::Relaxed);
        assert!(c.input_muted(false), "a muted microphone must hear nothing");
        assert!(c.input_muted(true));
        c.muted.store(false, Ordering::Relaxed);
        assert!(!c.input_muted(false), "unmuting must take effect at once");
    }

    #[test]
    fn speakers_gate_the_microphone_and_keep_gating_for_the_tail() {
        let level = Arc::new(AtomicU32::new(0.0f32.to_bits()));
        let c = ctx(Some(level.clone()), false);
        assert!(!c.input_muted(false), "silent speakers must not gate");

        level.store(0.5f32.to_bits(), Ordering::Relaxed);
        assert!(c.input_muted(false));

        // The speakers go quiet: the microphone stays gated for the tail, because
        // the audio it captured while they played arrives here a moment later.
        level.store(0.0f32.to_bits(), Ordering::Relaxed);
        assert!(c.input_muted(false), "the tail must still be gated");

        c.muted_until.set(Some(Instant::now() - Duration::from_millis(1)));
        assert!(!c.input_muted(false), "gate must reopen once the tail has passed");
    }

    #[test]
    fn assistant_speech_gates_hands_free_only() {
        let c = ctx(None, true);
        assert!(c.input_muted(true));
        assert!(!ctx(None, true).input_muted(false));
    }
}
