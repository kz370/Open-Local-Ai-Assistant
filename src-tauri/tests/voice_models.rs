//! End-to-end local voice test with real models (English, Arabic, German):
//! download (consented, checksum-verified) -> TTS synthesis -> STT transcription
//! -> automatic language detection.
//!
//! Skipped unless `LA_MODELS_DIR` points to a models directory, e.g.
//! `LA_MODELS_DIR=/path/to/models cargo test --test voice_models -- --nocapture`

use local_ai_assistant_lib::database::Db;
use local_ai_assistant_lib::services::hardware;
use local_ai_assistant_lib::services::language::Lang;
use local_ai_assistant_lib::services::models::{catalog, download, ModelStore};
use local_ai_assistant_lib::services::stt::SttService;
use local_ai_assistant_lib::services::tts::TtsService;
use local_ai_assistant_lib::settings::SettingsStore;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

const MODELS: &[&str] = &["whisper-small", "silero-vad", "kokoro-int8-en-v0_19", "piper-de_DE-thorsten-medium-int8", "piper-ar_JO-kareem-medium"];

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

#[tokio::test]
async fn tts_stt_roundtrip_three_languages() {
    let Ok(dir) = std::env::var("LA_MODELS_DIR") else {
        eprintln!("LA_MODELS_DIR not set; skipping real-model voice test");
        return;
    };
    let store = Arc::new(ModelStore::new(dir.into()));
    for id in MODELS {
        if !store.is_installed(id) {
            let m = catalog::find(id).unwrap();
            eprintln!("installing {id} ({} MB)…", m.download_size() / 1_000_000);
            download::install(&store, m, CancellationToken::new(), &|_| {}).await.expect("install");
        }
    }

    let db = Arc::new(Db::open_in_memory().unwrap());
    let settings = Arc::new(SettingsStore::load(db).unwrap());
    let hw = hardware::detect();
    let tts = TtsService::new(store.clone(), settings.clone(), hw.clone(), Arc::new(|_| {}));
    let stt = SttService::new(store.clone(), hw.clone());
    assert!(stt.vad_model_path().is_some());

    let cases = [
        (Lang::En, "Hello, how are you today? I would like to organize my files.", vec!["organize", "files"]),
        (Lang::De, "Guten Tag. Wie kann ich meine Dateien organisieren?", vec!["dateien", "organisieren"]),
        (Lang::Ar, "مرحبا، كيف حالك اليوم؟", vec!["اليوم"]),
    ];
    for (lang, text, expect_words) in cases {
        assert!(tts.is_available(lang), "no voice for {lang}");
        let t0 = std::time::Instant::now();
        let (samples, rate) = tts.synthesize(text, lang, hw.inference_threads()).expect("synthesize");
        let synth_ms = t0.elapsed().as_millis();
        assert!(samples.len() > rate as usize / 2, "audio too short");
        let resampler = sherpa_onnx::LinearResampler::create(rate as i32, 16_000).unwrap();
        let mut pcm = resampler.resample(&samples, true);
        pcm.extend(std::iter::repeat(0.0).take(8_000));

        let result = stt.transcribe(&pcm, &settings.get().stt).expect("transcribe");
        eprintln!("[{lang}] tts {synth_ms} ms, stt {} ms: {:?} -> detected {:?}", result.elapsed_ms, result.text, result.language);
        assert_eq!(result.language.as_deref(), Some(lang.code()), "language detection for {lang}");
        let got = words(&result.text);
        for w in expect_words {
            assert!(got.iter().any(|g| g.contains(&w.to_lowercase())), "[{lang}] expected '{w}' in {:?}", result.text);
        }
    }
}

/// Verifies a user-supplied model folder (any sherpa-onnx family), e.g.
/// `LA_STREAM_MODEL='H:\Models\openwhispr\parakeet-models\nemotron-3.5-asr-streaming-0.6b'`.
#[tokio::test]
async fn custom_model_folder_transcribes() {
    let (Ok(models_dir), Ok(model_path)) = (std::env::var("LA_MODELS_DIR"), std::env::var("LA_STREAM_MODEL")) else {
        eprintln!("LA_MODELS_DIR/LA_STREAM_MODEL not set; skipping custom-model test");
        return;
    };
    let model_dir = std::path::PathBuf::from(&model_path);
    let files = local_ai_assistant_lib::services::stt::engine::detect(&model_dir).expect("recognized model folder");
    eprintln!("detected family: {} (streaming: {})", files.family.label(), files.family.is_streaming());

    // Speak a sentence with the local TTS, then transcribe it with the user's model.
    let store = Arc::new(ModelStore::new(models_dir.into()));
    let db = Arc::new(Db::open_in_memory().unwrap());
    let settings = Arc::new(SettingsStore::load(db).unwrap());
    let hw = hardware::detect();
    let tts = TtsService::new(store.clone(), settings.clone(), hw.clone(), Arc::new(|_| {}));
    let text = "The assistant can transcribe speech with a local model.";
    let (samples, rate) = tts.synthesize(text, Lang::En, hw.inference_threads()).expect("synthesize");
    let mut pcm = sherpa_onnx::LinearResampler::create(rate as i32, 16_000).unwrap().resample(&samples, true);
    pcm.extend(std::iter::repeat(0.0).take(8_000));

    // The model lives outside the app folder: register it as an extra folder.
    store.set_extra_dirs(vec![model_dir.parent().unwrap().to_path_buf()]);
    let stt = SttService::new(store, hw);
    let mut stt_settings = settings.get().stt;
    stt_settings.model = model_dir.file_name().unwrap().to_string_lossy().to_string();
    assert!(stt.is_ready(&stt_settings), "model not discovered in the extra folder");

    let result = stt.transcribe(&pcm, &stt_settings).expect("transcribe");
    eprintln!("[{}] {} ms -> {:?}", result.model_id, result.elapsed_ms, result.text);
    let got = result.text.to_lowercase();
    for word in ["transcribe", "local", "model"] {
        assert!(got.contains(word), "expected '{word}' in {:?}", result.text);
    }
}

/// Drives the hands-free pipeline (VAD -> transcription -> events) with
/// recorded speech instead of a microphone.
#[tokio::test]
async fn hands_free_pipeline_transcribes_utterances() {
    let Ok(dir) = std::env::var("LA_MODELS_DIR") else {
        eprintln!("LA_MODELS_DIR not set; skipping hands-free pipeline test");
        return;
    };
    use local_ai_assistant_lib::services::audio::capture::CaptureEvent;
    use local_ai_assistant_lib::services::stt::session::{run_session_for_test, ListenMode, VoiceEvent};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    let store = Arc::new(ModelStore::new(dir.into()));
    for id in ["whisper-base", "silero-vad", "kokoro-int8-en-v0_19"] {
        if !store.is_installed(id) {
            let m = catalog::find(id).unwrap();
            download::install(&store, m, CancellationToken::new(), &|_| {}).await.expect("install");
        }
    }
    let db = Arc::new(Db::open_in_memory().unwrap());
    let settings = Arc::new(SettingsStore::load(db).unwrap());
    let hw = hardware::detect();
    let tts = TtsService::new(store.clone(), settings.clone(), hw.clone(), Arc::new(|_| {}));
    let stt = SttService::new(store.clone(), hw.clone());
    let vad_path = stt.vad_model_path().expect("silero vad installed");

    // Two spoken sentences with a pause between them.
    let (speech, rate) = tts.synthesize("Hello assistant, what is the weather today?", Lang::En, hw.inference_threads()).unwrap();
    let pcm = sherpa_onnx::LinearResampler::create(rate as i32, 16_000).unwrap().resample(&speech, true);

    let (tx, rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let events: Arc<Mutex<Vec<VoiceEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let stop_feeder = stop.clone();
    std::thread::spawn(move || {
        // Feed the audio in 20 ms chunks like the microphone would, then silence.
        for chunk in pcm.chunks(320) {
            let _ = tx.send(CaptureEvent::Samples(chunk.to_vec()));
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        for _ in 0..80 {
            let _ = tx.send(CaptureEvent::Samples(vec![0.0; 320]));
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        std::thread::sleep(std::time::Duration::from_secs(4));
        stop_feeder.store(true, Ordering::Relaxed);
    });

    let stt_settings = settings.get().stt;
    run_session_for_test(
        &stt,
        &stt_settings,
        ListenMode::HandsFree,
        &rx,
        Some(&vad_path),
        Arc::new(move |ev| sink.lock().unwrap().push(ev)),
        stop,
    );

    let seen = events.lock().unwrap();
    let transcripts: Vec<String> = seen
        .iter()
        .filter_map(|e| match e {
            VoiceEvent::Transcript { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    eprintln!("hands-free transcripts: {transcripts:?}");
    assert!(!transcripts.is_empty(), "hands-free produced no transcript; events: {:?}", seen.len());
    let joined = transcripts.join(" ").to_lowercase();
    assert!(joined.contains("weather"), "unexpected transcript: {joined}");
}

/// Checks that the default microphone actually delivers audio events.
/// Runs only when LA_MIC_TEST=1 (needs a real input device).
#[test]
fn microphone_delivers_audio_events() {
    if std::env::var("LA_MIC_TEST").ok().as_deref() != Some("1") {
        eprintln!("LA_MIC_TEST not set; skipping microphone capture test");
        return;
    }
    use local_ai_assistant_lib::services::audio::capture::{Capture, CaptureEvent};
    let (mut capture, rx) = Capture::start(None).expect("microphone opens");
    eprintln!("device: {}", capture.device_name);
    let mut samples = 0usize;
    let mut levels = 0usize;
    let mut peak = 0f32;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        match rx.recv_timeout(std::time::Duration::from_millis(300)) {
            Ok(CaptureEvent::Samples(s)) => samples += s.len(),
            Ok(CaptureEvent::Level(v, _)) => {
                levels += 1;
                peak = peak.max(v);
            }
            Ok(CaptureEvent::Error(e)) => panic!("capture error: {e}"),
            Err(_) => {}
        }
    }
    capture.stop();
    eprintln!("received {samples} samples and {levels} level updates in 2 s; peak level {peak:.3}");
    assert!(samples > 16_000, "microphone delivered too little audio: {samples} samples");
    assert!(levels > 10, "no level updates");
}

/// Full hands-free round trip: recorded speech -> transcript -> LM Studio ->
/// spoken reply. Needs LA_MODELS_DIR and a running LM Studio (LA_LIVE_LMSTUDIO=1).
#[tokio::test(flavor = "multi_thread")]
async fn hands_free_conversation_speaks_the_answer() {
    let (Ok(models_dir), Some("1")) = (std::env::var("LA_MODELS_DIR"), std::env::var("LA_LIVE_LMSTUDIO").ok().as_deref()) else {
        eprintln!("LA_MODELS_DIR/LA_LIVE_LMSTUDIO not set; skipping hands-free conversation test");
        return;
    };
    use local_ai_assistant_lib::services::ai::lmstudio::LmStudioService;
    use local_ai_assistant_lib::services::audio::capture::CaptureEvent;
    use local_ai_assistant_lib::services::chat::orchestrator::Emit;
    use local_ai_assistant_lib::services::chat::tools::NoTools;
    use local_ai_assistant_lib::services::chat::{ChatEngine, ChatEvent, ModelResolver, SendInput};
    use local_ai_assistant_lib::services::stt::session::{run_session_for_test, ListenMode, VoiceEvent};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    let store = Arc::new(ModelStore::new(models_dir.into()));
    let db = Arc::new(Db::open_in_memory().unwrap());
    let settings = Arc::new(SettingsStore::load(db.clone()).unwrap());
    settings.update(|s| s.tts.speak_responses = false).unwrap(); // voice turns must speak on their own
    let hw = hardware::detect();
    let tts = TtsService::new(store.clone(), settings.clone(), hw.clone(), Arc::new(|_| {}));
    let stt = Arc::new(SttService::new(store.clone(), hw.clone()));
    let vad = stt.vad_model_path().expect("vad installed");

    // Say something out loud (synthesised) and feed it to the session.
    let (speech, rate) = tts.synthesize("Please answer in one short sentence: what is two plus two?", Lang::En, hw.inference_threads()).unwrap();
    let pcm = sherpa_onnx::LinearResampler::create(rate as i32, 16_000).unwrap().resample(&speech, true);
    let (tx, rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_feeder = stop.clone();
    std::thread::spawn(move || {
        for chunk in pcm.chunks(320) {
            let _ = tx.send(CaptureEvent::Samples(chunk.to_vec()));
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        for _ in 0..100 {
            let _ = tx.send(CaptureEvent::Samples(vec![0.0; 320]));
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
        stop_feeder.store(true, Ordering::Relaxed);
    });

    let heard: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = heard.clone();
    let stt_settings = settings.get().stt;
    let stt2 = stt.clone();
    let listen = std::thread::spawn(move || {
        run_session_for_test(
            &stt2,
            &stt_settings,
            ListenMode::HandsFree,
            &rx,
            Some(&vad),
            Arc::new(move |ev| {
                if let VoiceEvent::Transcript { text, .. } = ev {
                    if !text.trim().is_empty() {
                        sink.lock().unwrap().push(text);
                    }
                }
            }),
            stop,
        );
    });
    listen.join().unwrap();
    let spoken = heard.lock().unwrap().clone();
    assert!(!spoken.is_empty(), "nothing was transcribed");
    eprintln!("user said: {:?}", spoken[0]);

    // The assistant answers that transcript as a voice turn.
    let ai = Arc::new(LmStudioService::new(&settings.get().ai.server_url, 300));
    let resolver = Arc::new(ModelResolver::with_hardware(ai.clone(), hw.clone()));
    let attachments = Arc::new(local_ai_assistant_lib::services::attachments::AttachmentStore::new(std::env::temp_dir().join("local-assistant-test-attachments")));
    let engine = ChatEngine::new(db, settings, ai, resolver, Arc::new(NoTools), tts.clone(), attachments);
    let events: Arc<Mutex<Vec<ChatEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let emit: Emit = Arc::new(move |ev| sink.lock().unwrap().push(ev));
    engine
        .send(
            SendInput { turn_id: "call-1".into(), conversation_id: None, text: spoken[0].clone(), spoken_language: Some("en".into()), voice: true, attachment_ids: vec![] },
            emit,
        )
        .await
        .expect("turn");

    let answer = events
        .lock()
        .unwrap()
        .iter()
        .find_map(|e| match e {
            ChatEvent::Done { message, .. } => Some(message.content.clone()),
            _ => None,
        })
        .expect("assistant answered");
    eprintln!("assistant said: {answer:?}");

    // The reply must actually be spoken: audio is queued or playing.
    let mut audible = false;
    for _ in 0..80 {
        if tts.has_audio() {
            audible = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    assert!(audible, "the assistant's voice reply was never queued for playback");
    eprintln!("assistant is speaking: {}", tts.is_speaking());
}
