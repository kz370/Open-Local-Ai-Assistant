//! Manual probe: live captions hear what the speakers play (WASAPI loopback)
//! and caption it, in more than one language.
//! Plays speech through the default output; run it with nothing else playing:
//! `LA_MODELS_DIR=... cargo test --test captions_probe -- --ignored --nocapture`

use local_ai_assistant_lib::database::Db;
use local_ai_assistant_lib::services::audio::playback::{Clip, Player};
use local_ai_assistant_lib::services::captions::{CaptionEvent, LiveCaptions};
use local_ai_assistant_lib::services::hardware;
use local_ai_assistant_lib::services::language::Lang;
use local_ai_assistant_lib::services::models::ModelStore;
use local_ai_assistant_lib::services::stt::SttService;
use local_ai_assistant_lib::services::tts::TtsService;
use local_ai_assistant_lib::settings::SettingsStore;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
#[ignore]
fn captions_system_audio_in_two_languages() {
    let dir = std::env::var("LA_MODELS_DIR").expect("LA_MODELS_DIR");
    let store = Arc::new(ModelStore::new(dir.into()));
    let db = Arc::new(Db::open_in_memory().unwrap());
    let settings = Arc::new(SettingsStore::load(db).unwrap());
    settings.update(|s| s.captions.model = "whisper-small".into()).unwrap();
    let hw = hardware::detect();
    let tts = TtsService::new(store.clone(), settings.clone(), hw.clone(), Arc::new(|_| {}));

    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let partials = Arc::new(Mutex::new(0usize));
    let (l2, p2) = (lines.clone(), partials.clone());
    let captions = LiveCaptions::new(
        Arc::new(SttService::new(store.clone(), hw.clone())),
        settings.clone(),
        Arc::new(move |ev| match ev {
            CaptionEvent::Line { text } => {
                eprintln!("line: {text}");
                l2.lock().unwrap().push(text);
            }
            CaptionEvent::Partial { text } => {
                eprintln!("  partial: {text}");
                *p2.lock().unwrap() += 1;
            }
            other => eprintln!("{other:?}"),
        }),
    );
    captions.start().expect("captions start");
    std::thread::sleep(Duration::from_secs(3)); // model load

    let player = Player::new(None);
    std::thread::sleep(Duration::from_millis(500));
    for (lang, text) in [
        (Lang::En, "Good evening. Tonight we are talking about the weather in the mountains, where it has been snowing for three days."),
        (Lang::De, "Guten Abend. Heute sprechen wir über das Wetter in den Bergen, wo es seit drei Tagen schneit."),
    ] {
        let (samples, rate) = tts.synthesize(text, lang, hw.inference_threads()).expect("synthesize");
        let secs = samples.len() as f32 / rate as f32;
        player.enqueue(Clip { samples, sample_rate: rate, tag: "probe".into() }).expect("enqueue");
        std::thread::sleep(Duration::from_secs_f32(secs + 1.0));
    }
    // Whisper on a CPU may need a few seconds for the last sentence.
    let heard = |words: &[&str]| {
        let all = lines.lock().unwrap().join(" ").to_lowercase();
        words.iter().any(|w| all.contains(w))
    };
    for _ in 0..150 {
        if heard(&["berg", "wetter", "abend"]) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    captions.stop();

    let all = lines.lock().unwrap().join(" ").to_lowercase();
    eprintln!("captions: {all}\npartials: {}", partials.lock().unwrap());
    assert!(all.contains("mountain") || all.contains("weather"), "English speech was not captioned");
    assert!(heard(&["berg", "wetter", "abend"]), "German speech was not captioned");
}
