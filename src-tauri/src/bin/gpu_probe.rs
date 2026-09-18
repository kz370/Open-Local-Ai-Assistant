//! Checks whether the CUDA GPU pack can actually run a voice model on this
//! machine. Copy it into the pack folder and run it from there:
//!   cargo build --bin gpu_probe
//!   copy target\debug\gpu_probe.exe <pack>\ && <pack>\gpu_probe.exe <model dir>
//! Prints the provider it asked for and how long synthesis took.

fn main() {
    let mut args = std::env::args().skip(1);
    let model_dir = std::path::PathBuf::from(args.next().expect("usage: gpu_probe <piper model dir> [provider]"));
    let provider = args.next().unwrap_or_else(|| "cuda".into());
    let onnx = std::fs::read_dir(&model_dir)
        .expect("model dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|x| x.to_str()) == Some("onnx"))
        .expect("no .onnx in the model dir");

    let p = |x: std::path::PathBuf| Some(x.to_string_lossy().to_string());
    let mut config = sherpa_onnx::OfflineTtsConfig::default();
    config.model.num_threads = 4;
    config.model.provider = Some(provider.clone());
    config.model.debug = true;
    config.max_num_sentences = 1;
    config.model.vits.model = p(onnx);
    config.model.vits.tokens = p(model_dir.join("tokens.txt"));
    config.model.vits.data_dir = p(model_dir.join("espeak-ng-data"));

    println!("provider requested: {provider}");
    let started = std::time::Instant::now();
    let tts = sherpa_onnx::OfflineTts::create(&config).expect("engine could not be created");
    println!("engine ready in {} ms", started.elapsed().as_millis());
    // Three runs: the first one pays for kernel warm-up, the later ones show
    // the speed the app would actually see.
    for run in 1..=3 {
        let started = std::time::Instant::now();
        let audio = tts
            .generate_with_config::<fn(&[f32], f32) -> bool>(
                "The quick brown fox jumps over the lazy dog, again and again.",
                &sherpa_onnx::GenerationConfig::default(),
                None,
            )
            .expect("synthesis failed");
        println!(
            "run {run}: {} samples at {} Hz in {} ms",
            audio.samples().len(),
            audio.sample_rate(),
            started.elapsed().as_millis()
        );
    }

    // Speech recognition on the same audio, which is the heavier of the two
    // models and the one most likely to gain from the GPU.
    let Some(stt_dir) = std::env::var_os("LA_STT_DIR").map(std::path::PathBuf::from) else { return };
    let audio = tts
        .generate_with_config::<fn(&[f32], f32) -> bool>(
            "The quick brown fox jumps over the lazy dog, again and again.",
            &sherpa_onnx::GenerationConfig::default(),
            None,
        )
        .expect("synthesis failed");
    let find = |needle: &str| {
        std::fs::read_dir(&stt_dir)
            .expect("stt dir")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.file_name().and_then(|n| n.to_str()).map(|n| n.contains(needle)).unwrap_or(false))
            .map(|p| p.to_string_lossy().to_string())
    };
    let mut cfg = sherpa_onnx::OfflineRecognizerConfig::default();
    cfg.model_config.tokens = find("tokens");
    cfg.model_config.num_threads = 4;
    cfg.model_config.provider = Some(provider.clone());
    cfg.model_config.whisper = sherpa_onnx::OfflineWhisperModelConfig {
        encoder: find("encoder"),
        decoder: find("decoder"),
        language: Some("en".into()),
        task: Some("transcribe".into()),
        tail_paddings: -1,
        ..Default::default()
    };
    cfg.decoding_method = Some("greedy_search".into());
    let started = std::time::Instant::now();
    let rec = sherpa_onnx::OfflineRecognizer::create(&cfg).expect("recognizer could not be created");
    println!("recognizer ready in {} ms", started.elapsed().as_millis());
    for run in 1..=3 {
        let started = std::time::Instant::now();
        let stream = rec.create_stream();
        stream.accept_waveform(audio.sample_rate(), audio.samples());
        rec.decode(&stream);
        let text = stream.get_result().map(|r| r.text.to_string()).unwrap_or_default();
        println!("stt run {run}: {} ms -> {text}", started.elapsed().as_millis());
    }
}
