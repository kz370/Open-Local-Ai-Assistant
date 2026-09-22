//! Neural text-to-speech: local sherpa-onnx voices (Kokoro / Piper VITS),
//! and the SILMA Arabic voice.
//!
//! Streaming: assistant text -> SentenceBuffer -> per-sentence language
//! detection -> voice selection -> synthesis worker -> playback queue.

pub mod sentence_buffer;
pub mod speech_text;
pub mod voices;

use crate::errors::{AppError, AppResult};
use crate::services::audio::playback::{Clip, Player};
use crate::services::chat::tools::SpeechSink;
use crate::services::hardware::HardwareInfo;
use crate::services::language::{detect, Lang};
use crate::services::models::catalog::Engine;
use crate::services::models::{find_file, ModelStore};
use crate::services::silma::Silma;
use crate::settings::SettingsStore;
use sentence_buffer::SentenceBuffer;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use voices::{list_voices, select_voice, VoiceInfo};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TtsEvent {
    Speaking { tag: String },
    /// A sentence started playing: what is being said right now and how long it
    /// takes, so the UI can follow the speech word by word.
    Sentence { tag: String, text: String, duration_ms: u64 },
    Paused,
    Resumed,
    Idle,
    VoiceUnavailable { language: String },
    Error { detail: String },
}

struct Job {
    generation: u64,
    tag: String,
    text: String,
    lang: Lang,
}

/// A clip handed to the player, kept until it has been played so the progress
/// watcher can tell the UI which sentence is being spoken.
struct QueuedSentence {
    tag: String,
    text: String,
    duration_ms: u64,
}

struct TurnState {
    buffer: SentenceBuffer,
    /// Language used when a sentence is too short to detect.
    last_lang: Option<Lang>,
    spoken: Vec<(String, Lang)>,
}

pub struct TtsService {
    store: Arc<ModelStore>,
    settings: Arc<SettingsStore>,
    pub player: Arc<Player>,
    jobs: Sender<Job>,
    /// Bumped by stop_all/begin; queued jobs from older generations are dropped.
    generation: Arc<AtomicU64>,
    turns: Mutex<HashMap<String, TurnState>>,
    /// Clip id -> the sentence that clip speaks.
    queued: Arc<Mutex<HashMap<u64, QueuedSentence>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
    last_turn: Mutex<Option<Vec<(String, Lang)>>>,
    engines: Arc<Mutex<HashMap<String, Arc<sherpa_onnx::OfflineTts>>>>,
    emit: Arc<dyn Fn(TtsEvent) + Send + Sync>,
    warned: Mutex<HashSet<(String, Lang)>>,
    /// Natural Arabic voice, when the user installed it.
    silma: OnceLock<Arc<Silma>>,
}

impl TtsService {
    pub fn new(store: Arc<ModelStore>, settings: Arc<SettingsStore>, hw: HardwareInfo, emit: Arc<dyn Fn(TtsEvent) + Send + Sync>) -> Arc<Self> {
        let s = settings.get();
        let player = Arc::new(Player::new(s.tts.output_device.clone()));
        player.set_volume(s.tts.volume);
        let (tx, rx) = mpsc::channel::<Job>();
        let svc = Arc::new(Self {
            store,
            settings,
            player,
            jobs: tx,
            generation: Arc::new(AtomicU64::new(0)),
            turns: Mutex::new(HashMap::new()),
            queued: Arc::new(Mutex::new(HashMap::new())),
            cancelled: Arc::new(Mutex::new(HashSet::new())),
            last_turn: Mutex::new(None),
            engines: Arc::new(Mutex::new(HashMap::new())),
            emit,
            warned: Mutex::new(HashSet::new()),
            silma: OnceLock::new(),
        });
        svc.clone().watch_playback();
        let weak = Arc::downgrade(&svc);
        let threads = hw.inference_threads().min(4);
        std::thread::Builder::new()
            .name("tts-synth".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    let Some(svc) = weak.upgrade() else { break };
                    if job.generation != svc.generation.load(Ordering::SeqCst)
                        || svc.cancelled.lock().unwrap_or_else(|p| p.into_inner()).contains(&job.tag)
                    {
                        continue;
                    }
                    if let Err(e) = svc.synthesize_and_queue(&job, threads) {
                        tracing::warn!(error = %e, "tts synthesis failed");
                        (svc.emit)(TtsEvent::Error { detail: e.to_string() });
                    }
                }
            })
            .expect("spawn tts thread");
        svc
    }

    /// Follows the player clip by clip and tells the UI which sentence is being
    /// spoken (and when speech has finished), which the enqueue-time `Speaking`
    /// event cannot do: clips are synthesized well before they are played.
    fn watch_playback(self: Arc<Self>) {
        let weak = Arc::downgrade(&self);
        drop(self);
        std::thread::Builder::new()
            .name("tts-progress".into())
            .spawn(move || {
                let mut last: u64 = 0;
                let mut spoke = false;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(40));
                    let Some(svc) = weak.upgrade() else { break };
                    let current = svc.player.current_clip();
                    if current == last {
                        continue;
                    }
                    last = current;
                    if current == 0 {
                        // Nothing playing any more: the queue ran dry on its own.
                        if spoke && !svc.player.is_active() {
                            spoke = false;
                            (svc.emit)(TtsEvent::Idle);
                        }
                        continue;
                    }
                    spoke = true;
                    let mut queued = svc.queued.lock().unwrap_or_else(|p| p.into_inner());
                    queued.retain(|id, _| *id >= current);
                    let Some(sentence) = queued.get(&current) else { continue };
                    (svc.emit)(TtsEvent::Sentence {
                        tag: sentence.tag.clone(),
                        text: sentence.text.clone(),
                        duration_ms: sentence.duration_ms,
                    });
                }
            })
            .expect("spawn tts progress thread");
    }

    pub fn set_silma(&self, silma: Arc<Silma>) {
        let _ = self.silma.set(silma);
    }

    pub fn voices(&self) -> Vec<VoiceInfo> {
        let mut voices = list_voices(&self.store.installed());
        if self.silma.get().is_some_and(|s| s.is_installed()) {
            voices.extend(voices::silma_voices());
        }
        voices
    }

    /// True when any language speaks through SILMA (so it is worth warming up).
    pub fn uses_silma(&self) -> bool {
        [Lang::En, Lang::Ar, Lang::De].into_iter().any(|l| self.voice_for(l).is_some_and(|v| v.engine == Engine::Silma))
    }

    pub fn is_available(&self, lang: Lang) -> bool {
        self.voice_for(lang).is_some()
    }

    /// The voice that will speak `lang`.
    pub fn selected_voice(&self, lang: Lang) -> Option<VoiceInfo> {
        self.voice_for(lang)
    }

    /// Ids of the ONNX voice models in memory.
    pub fn loaded_models(&self) -> Vec<String> {
        self.engines.lock().unwrap_or_else(|p| p.into_inner()).keys().cloned().collect()
    }

    /// Frees a voice model (a sentence being spoken keeps its own handle).
    pub fn unload_model(&self, model_id: &str) {
        self.engines.lock().unwrap_or_else(|p| p.into_inner()).remove(model_id);
    }

    /// Loads the ONNX voice for `lang` now (SILMA is started separately).
    pub fn preload_lang(&self, lang: Lang, threads: i32) -> AppResult<()> {
        let voice = self.voice_for(lang).ok_or_else(|| AppError::Tts(format!("no local voice installed for {}", lang.english_name())))?;
        if voice.engine == Engine::Silma {
            return Ok(());
        }
        self.engine(&voice.model_id, threads).map(|_| ())
    }

    fn voice_for(&self, lang: Lang) -> Option<VoiceInfo> {
        let s = self.settings.get();
        let pref = s.language.entries.iter().find(|e| e.code == lang.code()).map(|e| e.tts_voice.clone()).unwrap_or_else(|| "auto".into());
        select_voice(&self.voices(), lang, &pref, &s.tts.preferred_gender)
    }

    pub fn apply_settings(&self) {
        let s = self.settings.get().tts;
        self.player.set_volume(s.volume);
        self.player.set_device(s.output_device);
    }

    fn engine(&self, model_id: &str, threads: i32) -> AppResult<Arc<sherpa_onnx::OfflineTts>> {
        if let Some(e) = self.engines.lock().unwrap_or_else(|p| p.into_inner()).get(model_id) {
            return Ok(e.clone());
        }
        let model = self
            .store
            .find_installed(model_id)
            .ok_or_else(|| AppError::Tts(format!("voice model {model_id} is not installed")))?;
        let dir = &model.path;
        let path = |p: std::path::PathBuf| Some(p.to_string_lossy().to_string());
        let onnx = find_file(dir, |n| n.ends_with(".onnx")).ok_or_else(|| AppError::Tts("model file missing".into()))?;
        let tokens = find_file(dir, |n| n == "tokens.txt").ok_or_else(|| AppError::Tts("tokens.txt missing".into()))?;
        let data_dir = dir.join("espeak-ng-data");
        let mut config = sherpa_onnx::OfflineTtsConfig::default();
        config.model.num_threads = threads;
        let hw_pref = self.settings.get().tts.voice_hardware.get(model_id).cloned().unwrap_or_else(|| "auto".into());
        config.model.provider = Some(crate::services::gpu::provider_for(&hw_pref).into());
        config.max_num_sentences = 1;
        match model.engine {
            Engine::Kokoro => {
                config.model.kokoro.model = path(onnx);
                config.model.kokoro.voices = path(dir.join("voices.bin"));
                config.model.kokoro.tokens = path(tokens);
                config.model.kokoro.data_dir = path(data_dir);
            }
            Engine::Piper => {
                config.model.vits.model = path(onnx);
                config.model.vits.tokens = path(tokens);
                config.model.vits.data_dir = path(data_dir);
                config.model.vits.lexicon = dir.join("lexicon.txt").exists().then(|| dir.join("lexicon.txt").to_string_lossy().to_string());
            }
            Engine::Kitten => {
                config.model.kitten.model = path(onnx);
                config.model.kitten.voices = path(dir.join("voices.bin"));
                config.model.kitten.tokens = path(tokens);
                config.model.kitten.data_dir = path(data_dir);
            }
            _ => return Err(AppError::Tts("not a TTS model".into())),
        }
        let started = std::time::Instant::now();
        let tts = sherpa_onnx::OfflineTts::create(&config).ok_or_else(|| AppError::Tts(format!("failed to load voice model {model_id}")))?;
        tracing::info!(model = model_id, ms = started.elapsed().as_millis() as u64, "tts model loaded");
        let tts = Arc::new(tts);
        self.engines.lock().unwrap_or_else(|p| p.into_inner()).insert(model_id.into(), tts.clone());
        Ok(tts)
    }

    /// Synthesizes text to 16-bit-range float PCM (for tests / warm-up).
    pub fn synthesize(&self, text: &str, lang: Lang, threads: i32) -> AppResult<(Vec<f32>, u32)> {
        let voice = self.voice_for(lang).ok_or_else(|| AppError::Tts(format!("no local voice installed for {}", lang.english_name())))?;
        let speed = self.settings.get().tts.speed;
        if voice.engine == Engine::Silma {
            let silma = self.silma.get().ok_or_else(|| AppError::Tts("SILMA is not available".into()))?;
            return silma.synthesize(text, speed);
        }
        let engine = self.engine(&voice.model_id, threads)?;
        let gen = sherpa_onnx::GenerationConfig { speed, sid: voice.speaker_id, ..Default::default() };
        let audio = engine
            .generate_with_config::<fn(&[f32], f32) -> bool>(text, &gen, None)
            .ok_or_else(|| AppError::Tts("synthesis failed".into()))?;
        Ok((audio.samples().to_vec(), audio.sample_rate() as u32))
    }

    fn synthesize_and_queue(&self, job: &Job, threads: i32) -> AppResult<()> {
        // Reaching synthesis means the speech libraries loaded and ran.
        crate::services::gpu::mark_healthy();
        let Some(_voice) = self.voice_for(job.lang) else {
            let key = (job.tag.clone(), job.lang);
            if self.warned.lock().unwrap_or_else(|p| p.into_inner()).insert(key) {
                (self.emit)(TtsEvent::VoiceUnavailable { language: job.lang.code().into() });
            }
            return Ok(());
        };
        let (samples, rate) = self.synthesize(&job.text, job.lang, threads)?;
        if samples.is_empty() {
            return Ok(());
        }
        if job.generation != self.generation.load(Ordering::SeqCst)
            || self.cancelled.lock().unwrap_or_else(|p| p.into_inner()).contains(&job.tag)
        {
            return Ok(());
        }
        (self.emit)(TtsEvent::Speaking { tag: job.tag.clone() });
        let duration_ms = samples.len() as u64 * 1000 / rate.max(1) as u64;
        let id = self.player.enqueue(Clip { samples, sample_rate: rate, tag: job.tag.clone() })?;
        self.queued.lock().unwrap_or_else(|p| p.into_inner()).insert(
            id,
            QueuedSentence { tag: job.tag.clone(), text: job.text.clone(), duration_ms },
        );
        Ok(())
    }

    fn sentence_lang(&self, sentence: &str, fallback: Option<Lang>) -> Lang {
        detect(sentence)
            .filter(|d| d.confidence >= 0.5 || fallback.is_none())
            .map(|d| d.lang)
            .or(fallback)
            .unwrap_or(Lang::En)
    }

    fn send_job(&self, tag: &str, text: String, lang: Lang) {
        let generation = self.generation.load(Ordering::SeqCst);
        let _ = self.jobs.send(Job { generation, tag: tag.into(), text, lang });
    }

    fn queue_sentence(&self, tag: &str, sentence: String, fallback: Option<Lang>) -> Lang {
        let lang = self.sentence_lang(&sentence, fallback);
        self.send_job(tag, sentence, lang);
        lang
    }

    /// Speaks arbitrary text (Test Voice, replay of a message).
    pub fn speak(&self, tag: &str, text: &str, lang_hint: Option<Lang>) {
        self.stop_all();
        self.cancelled.lock().unwrap_or_else(|p| p.into_inner()).remove(tag);
        let mut buf = SentenceBuffer::default();
        let mut sentences = buf.push(text);
        sentences.extend(buf.flush());
        let mut last = lang_hint;
        for s in sentences {
            last = Some(self.queue_sentence(tag, s, last));
        }
    }

    pub fn replay_last(&self) -> bool {
        let Some(turn) = self.last_turn.lock().unwrap_or_else(|p| p.into_inner()).clone() else { return false };
        self.stop_all();
        let tag = format!("replay-{}", uuid::Uuid::new_v4().simple());
        let generation = self.generation.load(Ordering::SeqCst);
        for (text, lang) in turn {
            let _ = self.jobs.send(Job { generation, tag: tag.clone(), text, lang });
        }
        true
    }

    pub fn stop_all(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.turns.lock().unwrap_or_else(|p| p.into_inner()).clear();
        self.queued.lock().unwrap_or_else(|p| p.into_inner()).clear();
        self.player.stop(None);
        (self.emit)(TtsEvent::Idle);
    }

    pub fn is_speaking(&self) -> bool {
        self.player.is_active() && !self.player.is_paused()
    }

    /// Pauses or resumes the current speech without losing the queue.
    pub fn set_paused(&self, paused: bool) {
        self.player.set_paused(paused);
        (self.emit)(if paused { TtsEvent::Paused } else { TtsEvent::Resumed });
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused()
    }

    /// True when speech is playing or queued (even while paused).
    pub fn has_audio(&self) -> bool {
        self.player.is_active()
    }
}

impl SpeechSink for TtsService {
    fn begin(&self, turn_id: &str) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.player.stop(None);
        self.turns.lock().unwrap_or_else(|p| p.into_inner()).insert(
            turn_id.into(),
            TurnState { buffer: SentenceBuffer::default(), last_lang: None, spoken: Vec::new() },
        );
    }

    fn push_text(&self, turn_id: &str, text: &str) {
        // "Speak after the reply" collects sentences here and queues them in finish().
        let hold = self.settings.get().tts.speak_after_reply;
        let mut turns = self.turns.lock().unwrap_or_else(|p| p.into_inner());
        let Some(state) = turns.get_mut(turn_id) else { return };
        for sentence in state.buffer.push(text) {
            let lang = if hold {
                self.sentence_lang(&sentence, state.last_lang)
            } else {
                self.queue_sentence(turn_id, sentence.clone(), state.last_lang)
            };
            state.last_lang = Some(lang);
            state.spoken.push((sentence, lang));
        }
    }

    fn finish(&self, turn_id: &str) {
        let hold = self.settings.get().tts.speak_after_reply;
        let mut turns = self.turns.lock().unwrap_or_else(|p| p.into_inner());
        let Some(mut state) = turns.remove(turn_id) else { return };
        for sentence in state.buffer.flush() {
            let lang = self.sentence_lang(&sentence, state.last_lang);
            state.last_lang = Some(lang);
            if !hold {
                self.send_job(turn_id, sentence.clone(), lang);
            }
            state.spoken.push((sentence, lang));
        }
        if hold {
            // Nothing was queued while the reply streamed; speak all of it now.
            for (sentence, lang) in &state.spoken {
                self.send_job(turn_id, sentence.clone(), *lang);
            }
        }
        *self.last_turn.lock().unwrap_or_else(|p| p.into_inner()) = Some(state.spoken);
    }

    fn cancel(&self, turn_id: &str) {
        self.turns.lock().unwrap_or_else(|p| p.into_inner()).remove(turn_id);
        self.cancelled.lock().unwrap_or_else(|p| p.into_inner()).insert(turn_id.into());
        self.player.stop(Some(turn_id));
    }
}
