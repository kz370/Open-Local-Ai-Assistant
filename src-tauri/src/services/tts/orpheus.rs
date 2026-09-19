//! Orpheus voices: an LLM served by LM Studio writes SNAC audio codes as
//! `<custom_token_N>` text, and the SNAC decoder (ONNX) turns them into sound.
//!
//! One Orpheus model speaks one language, so each language names its own
//! LM Studio model in settings. The model understands expressive tags such as
//! `<laugh>` and `<sigh>` inside the text it reads.

use crate::errors::{AppError, AppResult};
use crate::services::ai::lmstudio::LmStudioService;
use crate::services::models::{find_file, ModelStore};
use super::cache::{self, AudioCache, CacheInfo};
use regex::Regex;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

/// Catalog id of the SNAC 24 kHz decoder.
pub const SNAC_MODEL_ID: &str = "snac-24khz-decoder";
pub const SAMPLE_RATE: u32 = 24_000;

/// Expressive tags Orpheus performs instead of reading aloud.
pub const EXPRESSIVE_TAGS: &[&str] = &["laugh", "chuckle", "sigh", "cough", "sniffle", "groan", "yawn", "gasp"];

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i)\s*<(?:{})>\s*", EXPRESSIVE_TAGS.join("|"))).unwrap());
static CUSTOM_TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<custom_token_(\d+)>").unwrap());

/// Removes expressive tags, for voices that would read them out.
pub fn strip_expressive_tags(text: &str) -> String {
    TAG.replace_all(text, " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Voices each Orpheus model was fine-tuned on: (name, gender).
pub fn voices_for(lang: &str) -> &'static [(&'static str, &'static str)] {
    match lang {
        "en" => &[("tara", "female"), ("leah", "female"), ("jess", "female"), ("leo", "male"), ("dan", "male"), ("mia", "female"), ("zac", "male"), ("zoe", "female")],
        "de" => &[("jana", "female"), ("thomas", "male"), ("max", "male")],
        _ => &[],
    }
}

/// Turns the model's `<custom_token_N>` output into SNAC codes: one frame is
/// seven tokens, each position offset by its own 4096-code codebook.
/// Returns the three SNAC layers (1, 2 and 4 codes per frame).
pub fn tokens_to_codes(text: &str) -> [Vec<i64>; 3] {
    let mut ids = Vec::new();
    for cap in CUSTOM_TOKEN.captures_iter(text) {
        let Ok(n) = cap[1].parse::<i64>() else { continue };
        // Control tokens (start/end of speech) fall below zero and are skipped.
        let id = n - 10 - (ids.len() as i64 % 7) * 4096;
        if id > 0 {
            ids.push(id);
        }
    }
    let mut layers: [Vec<i64>; 3] = Default::default();
    for f in ids.chunks_exact(7) {
        if f.iter().any(|c| !(0..4096).contains(c)) {
            continue;
        }
        layers[0].push(f[0]);
        layers[1].extend([f[1], f[4]]);
        layers[2].extend([f[2], f[3], f[5], f[6]]);
    }
    layers
}

pub struct Orpheus {
    lmstudio: Arc<LmStudioService>,
    store: Arc<ModelStore>,
    decoder: Mutex<Option<ort::session::Session>>,
    cache: AudioCache,
}

impl Orpheus {
    pub fn new(lmstudio: Arc<LmStudioService>, store: Arc<ModelStore>, cache_dir: PathBuf) -> Self {
        Self { lmstudio, store, decoder: Mutex::new(None), cache: AudioCache::new(cache_dir) }
    }

    /// How much speech is kept on disk.
    pub fn cache_info(&self) -> CacheInfo {
        self.cache.info()
    }

    /// Forgets every saved clip; they are generated again when next spoken.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    pub fn decoder_installed(&self) -> bool {
        self.store.find_installed(SNAC_MODEL_ID).is_some()
    }

    /// Loads the SNAC decoder now instead of on the first sentence.
    pub fn preload(&self) -> AppResult<()> {
        let mut guard = self.decoder.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_none() {
            *guard = Some(self.load_decoder()?);
        }
        Ok(())
    }

    pub fn unload(&self) {
        *self.decoder.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    pub fn is_loaded(&self) -> bool {
        self.decoder.lock().unwrap_or_else(|p| p.into_inner()).is_some()
    }

    fn load_decoder(&self) -> AppResult<ort::session::Session> {
        let model = self
            .store
            .find_installed(SNAC_MODEL_ID)
            .ok_or_else(|| AppError::Tts("the SNAC decoder for Orpheus voices is not installed".into()))?;
        let file = find_file(&model.path, |n| n.ends_with(".onnx")).ok_or_else(|| AppError::Tts("SNAC decoder file missing".into()))?;
        load_session(&file)
    }

    /// Speaks `text` with `voice` through the LM Studio `model`. Blocking.
    /// Re-uses a saved clip when there is one, and keeps the cache under
    /// `cache_bytes` (0 = do not save anything).
    pub fn synthesize(&self, model: &str, voice: &str, text: &str, cache_bytes: u64) -> AppResult<(Vec<f32>, u32)> {
        let key = cache::key(model, voice, text);
        if cache_bytes > 0 {
            if let Some(audio) = self.cache.get(&key) {
                tracing::debug!(model, "orpheus sentence from cache");
                return Ok(audio);
            }
        }
        let started = std::time::Instant::now();
        let prompt = format!("<|audio|>{voice}: {text}<|eot_id|>");
        // About 85 tokens per second of speech; leave room for slow talkers.
        let max_tokens = (text.chars().count() * 12 + 300).clamp(600, 4096);
        let body = json!({
            "model": model,
            "prompt": prompt,
            "max_tokens": max_tokens,
            "temperature": 0.6,
            "top_p": 0.9,
            "repeat_penalty": 1.1,
            "stop": ["<custom_token_2>"],
            "stream": false,
        });
        let lm = self.lmstudio.clone();
        let out = tauri::async_runtime::block_on(async move { lm.complete_text(body).await })?;
        let generated_ms = started.elapsed().as_millis() as u64;
        let codes = tokens_to_codes(&out);
        if codes[0].is_empty() {
            return Err(AppError::Tts(format!("{model} returned no audio; is it an Orpheus model?")));
        }
        let samples = self.decode(codes)?;
        tracing::debug!(model, frames = samples.len() / 2048, generated_ms, total_ms = started.elapsed().as_millis() as u64, "orpheus sentence");
        if let Err(e) = self.cache.put(&key, &samples, SAMPLE_RATE, cache_bytes) {
            tracing::warn!(error = %e, "could not save spoken audio");
        }
        Ok((samples, SAMPLE_RATE))
    }

    fn decode(&self, codes: [Vec<i64>; 3]) -> AppResult<Vec<f32>> {
        let mut guard = self.decoder.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_none() {
            *guard = Some(self.load_decoder()?);
        }
        decode_with(guard.as_mut().expect("decoder loaded"), codes)
    }
}

fn tts_err(e: impl std::fmt::Display) -> AppError {
    AppError::Tts(format!("SNAC decoder: {e}"))
}

/// Points `ort` at the ONNX Runtime DLL sherpa-onnx already loaded (next to
/// the executable), once per process.
fn init_runtime() -> AppResult<()> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        let dir = std::env::current_exe().map_err(|e| e.to_string())?.parent().map(PathBuf::from).ok_or("no executable directory")?;
        let lib = if cfg!(windows) { "onnxruntime.dll" } else if cfg!(target_os = "macos") { "libonnxruntime.dylib" } else { "libonnxruntime.so" };
        ort::init_from(dir.join(lib)).map_err(|e| e.to_string())?.commit();
        Ok(())
    })
    .clone()
    .map_err(tts_err)
}

fn load_session(file: &std::path::Path) -> AppResult<ort::session::Session> {
    init_runtime()?;
    let started = std::time::Instant::now();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 4);
    let session = ort::session::Session::builder()
        .map_err(tts_err)?
        .with_intra_threads(threads)
        .map_err(tts_err)?
        .commit_from_file(file)
        .map_err(tts_err)?;
    tracing::info!(ms = started.elapsed().as_millis() as u64, "SNAC decoder loaded");
    Ok(session)
}

fn decode_with(session: &mut ort::session::Session, codes: [Vec<i64>; 3]) -> AppResult<Vec<f32>> {
    use ort::value::Tensor;
    let [c0, c1, c2] = codes;
    let t = |v: Vec<i64>| Tensor::from_array(([1usize, v.len()], v)).map_err(tts_err);
    let outputs = session.run(ort::inputs![t(c0)?, t(c1)?, t(c2)?]).map_err(tts_err)?;
    let (_, audio) = outputs[0].try_extract_tensor::<f32>().map_err(tts_err)?;
    Ok(audio.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(n: i64) -> String {
        format!("<custom_token_{n}>")
    }

    #[test]
    fn parses_frames_into_snac_layers() {
        // Two frames: position p of each frame carries code (10*frame + p).
        let mut text = String::from("<custom_token_1>"); // start of speech: skipped
        for frame in 0..2i64 {
            for p in 0..7i64 {
                text.push_str(&token(10 * frame + p + 1 + 10 + p * 4096));
            }
        }
        text.push_str(&token(5)); // trailing partial token: ignored
        let [a, b, c] = tokens_to_codes(&text);
        assert_eq!(a, vec![1, 11]);
        assert_eq!(b, vec![2, 5, 12, 15]);
        assert_eq!(c, vec![3, 4, 6, 7, 13, 14, 16, 17]);
    }

    #[test]
    fn expressive_tags() {
        assert_eq!(strip_expressive_tags("Well <laugh> that was fun. <SIGH>"), "Well that was fun.");
        assert_eq!(strip_expressive_tags("a <b> c"), "a <b> c");
        assert_eq!(voices_for("en")[0].0, "tara");
        assert!(voices_for("ar").is_empty());
    }

    /// Decodes random codes with a real SNAC decoder:
    /// `SNAC_ONNX=path/to/decoder_model.onnx cargo test snac_decodes -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn snac_decodes() {
        let path = std::env::var("SNAC_ONNX").expect("SNAC_ONNX");
        let mut session = load_session(std::path::Path::new(&path)).unwrap();
        println!("inputs: {:?}", session.inputs().iter().map(|i| i.name().to_string()).collect::<Vec<_>>());
        let frames = 24; // about 2 seconds
        let codes = [(0..frames).map(|i| i * 37 % 4096).collect(), (0..frames * 2).map(|i| i * 91 % 4096).collect(), (0..frames * 4).map(|i| i * 13 % 4096).collect()];
        let started = std::time::Instant::now();
        let audio = decode_with(&mut session, codes).unwrap();
        println!("{} samples in {} ms", audio.len(), started.elapsed().as_millis());
        assert_eq!(audio.len(), frames as usize * 2048);
    }

    /// `SNAC_ONNX=... cargo test snac_chunk_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn snac_chunk_cost() {
        let mut session = load_session(std::path::Path::new(&std::env::var("SNAC_ONNX").expect("SNAC_ONNX"))).unwrap();
        for frames in [4i64, 8, 14, 26, 50] {
            let codes = [(0..frames).map(|i| i * 37 % 4096).collect(), (0..frames * 2).map(|i| i * 91 % 4096).collect(), (0..frames * 4).map(|i| i * 13 % 4096).collect()];
            let started = std::time::Instant::now();
            let audio = decode_with(&mut session, codes).unwrap();
            let ms = started.elapsed().as_secs_f32() * 1000.0;
            println!("{frames:>3} frames ({:.2}s audio): {ms:.0} ms", audio.len() as f32 / SAMPLE_RATE as f32);
        }
    }

    /// Full round trip through a running LM Studio; writes `ORPHEUS_WAV`:
    /// `ORPHEUS_MODEL=<id> SNAC_ONNX=<path> ORPHEUS_WAV=out.wav cargo test orpheus_speaks -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn orpheus_speaks() {
        let model = std::env::var("ORPHEUS_MODEL").expect("ORPHEUS_MODEL");
        let lm = LmStudioService::new("http://localhost:1234/v1", 120);
        let text = "Hey there! <laugh> I did not expect that to work on the first try.";
        let body = json!({ "model": model, "prompt": format!("<|audio|>tara: {text}<|eot_id|>"), "max_tokens": 1500,
            "temperature": 0.6, "top_p": 0.9, "repeat_penalty": 1.1, "stop": ["<custom_token_2>"], "stream": false });
        let started = std::time::Instant::now();
        let out = tauri::async_runtime::block_on(lm.complete_text(body)).unwrap();
        let codes = tokens_to_codes(&out);
        println!("{} frames generated in {} ms", codes[0].len(), started.elapsed().as_millis());
        let mut session = load_session(std::path::Path::new(&std::env::var("SNAC_ONNX").expect("SNAC_ONNX"))).unwrap();
        let audio = decode_with(&mut session, codes).unwrap();
        println!("{:.1} s of audio after {} ms", audio.len() as f32 / SAMPLE_RATE as f32, started.elapsed().as_millis());
        let pcm: Vec<u8> = audio.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
        let mut wav = Vec::new();
        for part in [&b"RIFF"[..], &(36 + pcm.len() as u32).to_le_bytes(), b"WAVEfmt ", &16u32.to_le_bytes(), &1u16.to_le_bytes(), &1u16.to_le_bytes(),
            &SAMPLE_RATE.to_le_bytes(), &(SAMPLE_RATE * 2).to_le_bytes(), &2u16.to_le_bytes(), &16u16.to_le_bytes(), b"data", &(pcm.len() as u32).to_le_bytes(), &pcm] {
            wav.extend_from_slice(part);
        }
        std::fs::write(std::env::var("ORPHEUS_WAV").expect("ORPHEUS_WAV"), wav).unwrap();
    }
}
