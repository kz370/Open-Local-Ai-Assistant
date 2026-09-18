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
    let started = std::time::Instant::now();
    let audio = tts
        .generate_with_config::<fn(&[f32], f32) -> bool>(
            "The quick brown fox jumps over the lazy dog, again and again.",
            &sherpa_onnx::GenerationConfig::default(),
            None,
        )
        .expect("synthesis failed");
    println!("synthesized {} samples at {} Hz in {} ms", audio.samples().len(), audio.sample_rate(), started.elapsed().as_millis());
}
