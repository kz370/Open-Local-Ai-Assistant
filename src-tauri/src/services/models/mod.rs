//! Local voice model management: discovery of installed models (catalog,
//! manually placed, or in extra folders the user points at), hardware-based
//! recommendations and consented downloads.

pub mod catalog;
pub mod download;

use crate::services::hardware::HardwareInfo;
use crate::services::stt::engine;
use catalog::{CatalogModel, Engine, ModelKind, CATALOG};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const COMPLETE_MARKER: &str = ".complete";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModel {
    pub id: String,
    pub name: String,
    pub kind: ModelKind,
    pub engine: Engine,
    pub languages: Vec<String>,
    pub path: PathBuf,
    /// "catalog" | "custom"
    pub source: String,
    /// Speech-model family ("Whisper", "Streaming transducer", …).
    pub family: Option<String>,
    /// True for models that produce live text while you speak.
    pub streaming: bool,
    /// "female" | "male" | "mixed" | "" (voices only).
    pub gender: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub stt: &'static str,
    pub vad: &'static str,
    pub tts_en: &'static str,
    pub tts_de: &'static str,
    pub tts_ar: &'static str,
}

impl Recommendation {
    pub fn ids(&self) -> Vec<&'static str> {
        vec![self.stt, self.vad, self.tts_en, self.tts_de, self.tts_ar]
    }
}

/// Starter set: the smallest models that still work well, so the first
/// download is quick. Bigger models can be installed from Settings later.
pub fn recommend(_hw: &HardwareInfo) -> Recommendation {
    Recommendation {
        stt: "whisper-base",
        vad: "silero-vad",
        tts_en: "kitten-nano-en-v0_8-int8",
        tts_de: "piper-de_DE-thorsten-medium-int8",
        tts_ar: "nabra-82m-arabic-int8",
    }
}

pub struct ModelStore {
    pub dir: PathBuf,
    /// Extra folders the user added (models there are used read-only).
    extra_dirs: std::sync::RwLock<Vec<PathBuf>>,
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| match e.metadata() {
                    Ok(m) if m.is_dir() => dir_size(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                })
                .sum()
        })
        .unwrap_or(0)
}

impl ModelStore {
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self { dir, extra_dirs: std::sync::RwLock::new(Vec::new()) }
    }

    pub fn set_extra_dirs(&self, dirs: Vec<PathBuf>) {
        *self.extra_dirs.write().unwrap_or_else(|p| p.into_inner()) = dirs;
    }

    pub fn extra_dirs(&self) -> Vec<PathBuf> {
        self.extra_dirs.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn model_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.model_dir(id).join(COMPLETE_MARKER).exists()
    }

    /// Catalog models that finished installing, plus any compatible model
    /// folder inside the models folder or the user's extra folders.
    pub fn installed(&self) -> Vec<InstalledModel> {
        let mut out: Vec<InstalledModel> = CATALOG
            .iter()
            .filter(|m| self.is_installed(m.id))
            .map(|m| from_catalog(m, self.model_dir(m.id)))
            .collect();
        self.scan_dir(&self.dir, true, &mut out);
        for extra in self.extra_dirs() {
            self.scan_dir(&extra, false, &mut out);
        }
        out
    }

    /// Recursively scans a folder for model directories. Handles plain model
    /// folders, grouped folders and the Hugging Face cache layout
    /// (`models--org--name/snapshots/<hash>/`).
    fn scan_dir(&self, root: &Path, skip_catalog: bool, out: &mut Vec<InstalledModel>) {
        const MAX_DEPTH: usize = 4;
        fn walk(store: &ModelStore, dir: &Path, depth: usize, skip_catalog: bool, out: &mut Vec<InstalledModel>) {
            let name = folder_name(dir);
            if let Some(m) = detect_custom(dir, &name) {
                if !out.iter().any(|x| x.path == m.path) {
                    out.push(m);
                }
                return; // a model folder is a leaf
            }
            if depth >= MAX_DEPTH {
                return;
            }
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for e in entries.flatten() {
                let path = e.path();
                let child = e.file_name().to_string_lossy().to_string();
                if !path.is_dir() || child.starts_with('.') || SKIP_DIRS.contains(&child.as_str()) {
                    continue;
                }
                if skip_catalog && depth == 0 && catalog::find(&child).is_some() {
                    continue;
                }
                walk(store, &path, depth + 1, skip_catalog, out);
            }
        }
        walk(self, root, 0, skip_catalog, out);
    }

    /// Model folders found in the user's extra folders that this app cannot
    /// run, with the reason (e.g. PyTorch weights that need an ONNX export).
    pub fn incompatible(&self) -> Vec<IncompatibleModel> {
        let mut out = Vec::new();
        for dir in self.extra_dirs() {
            scan_incompatible(&dir, 0, &mut out);
        }
        out
    }

    pub fn find_installed(&self, id: &str) -> Option<InstalledModel> {
        self.installed().into_iter().find(|m| m.id == id)
    }

    pub fn delete(&self, id: &str) -> std::io::Result<()> {
        let dir = self.model_dir(id);
        // Guard against path traversal through crafted ids; only models inside
        // the app's own folder can be deleted.
        if id.contains(['/', '\\']) || id.contains("..") || !dir.starts_with(&self.dir) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid model id"));
        }
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }
}

/// Folders that never contain a usable model on their own.
const SKIP_DIRS: &[&str] = &["espeak-ng-data", "test_wavs", "blobs", "refs", ".git", "dict", "node_modules"];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IncompatibleModel {
    pub name: String,
    pub path: PathBuf,
    /// Short, user-facing reason.
    pub reason: String,
}

fn scan_incompatible(dir: &Path, depth: usize, out: &mut Vec<IncompatibleModel>) {
    if depth > 4 || out.len() > 50 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if e.path().is_dir() {
            if !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_str()) {
                subdirs.push(e.path());
            }
        } else {
            files.push(name.to_ascii_lowercase());
        }
    }
    let has_onnx = files.iter().any(|f| f.ends_with(".onnx"));
    let torch = files.iter().any(|f| f.ends_with(".safetensors") || f.ends_with(".pt") || f.ends_with(".pth") || f.ends_with(".ckpt") || f == "pytorch_model.bin");
    let gguf = files.iter().any(|f| f.ends_with(".gguf"));
    if !has_onnx && (torch || gguf) {
        out.push(IncompatibleModel {
            name: display_name(dir, &folder_name(dir)),
            path: dir.to_path_buf(),
            reason: if gguf {
                "GGUF models run in LM Studio, not in the local speech engine".into()
            } else {
                "PyTorch weights: this app needs an ONNX export (sherpa-onnx format)".into()
            },
        });
        return;
    }
    if has_onnx {
        return; // handled by the normal scan
    }
    for d in subdirs {
        scan_incompatible(&d, depth + 1, out);
    }
}

fn folder_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

fn from_catalog(m: &CatalogModel, path: PathBuf) -> InstalledModel {
    InstalledModel {
        id: m.id.into(),
        name: m.name.into(),
        kind: m.kind,
        engine: m.engine,
        languages: m.languages.iter().map(|s| s.to_string()).collect(),
        source: "catalog".into(),
        family: (m.kind == ModelKind::Stt).then(|| "Whisper".to_string()),
        streaming: false,
        gender: m.gender.into(),
        size_bytes: dir_size(&path),
        path,
    }
}

fn files_with(dir: &Path, pred: impl Fn(&str) -> bool) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|it| it.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| pred(n)).collect())
        .unwrap_or_default()
}

/// Guesses the voice gender from a Piper voice folder/file name.
fn piper_gender(name: &str) -> &'static str {
    const FEMALE: &[&str] = &["eva", "kerstin", "ramona", "amy", "jenny", "female", "lessac", "dii", "hfc_female", "ljspeech"];
    const MALE: &[&str] = &["thorsten", "kareem", "karlsson", "pavoque", "miro", "ryan", "joe", "male", "alan", "danny"];
    let n = name.to_ascii_lowercase();
    if FEMALE.iter().any(|k| n.contains(k)) {
        "female"
    } else if MALE.iter().any(|k| n.contains(k)) {
        "male"
    } else {
        ""
    }
}

/// Friendly name for a folder, resolving Hugging Face cache layouts
/// (`models--org--name/snapshots/<hash>` -> `org/name`).
pub fn display_name(dir: &Path, fallback: &str) -> String {
    let is_snapshot = dir.parent().map(|p| folder_name(p) == "snapshots").unwrap_or(false);
    if is_snapshot {
        if let Some(repo) = dir.parent().and_then(|p| p.parent()).map(folder_name) {
            if let Some(rest) = repo.strip_prefix("models--") {
                return rest.replace("--", "/");
            }
            return repo;
        }
    }
    fallback.to_string()
}

/// Recognizes a model folder: any sherpa-onnx speech model, VAD, or voice.
pub fn detect_custom(dir: &Path, name: &str) -> Option<InstalledModel> {
    let name = &display_name(dir, name);
    let path_hint = dir.to_string_lossy().to_ascii_lowercase();
    let has = |f: &str| dir.join(f).exists();
    let onnx = files_with(dir, |n| n.ends_with(".onnx"));
    let tokens = files_with(dir, |n| n.ends_with("tokens.txt"));
    if onnx.is_empty() {
        return None;
    }
    let base = |kind, eng, langs: Vec<String>, family: Option<String>, streaming: bool, gender: &str| InstalledModel {
        id: name.to_string(),
        name: name.to_string(),
        kind,
        engine: eng,
        languages: langs,
        path: dir.to_path_buf(),
        source: "custom".into(),
        family,
        streaming,
        gender: gender.to_string(),
        size_bytes: dir_size(dir),
    };

    if onnx.iter().any(|n| n.starts_with("silero_vad") || n.starts_with("ten-vad")) {
        return Some(base(ModelKind::Vad, Engine::SileroVad, vec!["*".into()], None, false, ""));
    }
    if has("voices.bin") && !tokens.is_empty() {
        // Kitten and Kokoro both ship a voices.bin; the folder name tells them apart.
        let engine = if path_hint.contains("kitten") { Engine::Kitten } else { Engine::Kokoro };
        let lang = if path_hint.contains("arab") || path_hint.contains("nabra") || path_hint.contains("-ar") {
            "ar"
        } else if path_hint.contains("multi-lang") || path_hint.contains("multilang") {
            "*"
        } else {
            "en"
        };
        return Some(base(ModelKind::Tts, engine, vec![lang.into()], None, false, "mixed"));
    }
    if !tokens.is_empty() && (has("espeak-ng-data") || has("lexicon.txt")) {
        // Piper voices are named like "de_DE-thorsten-high.onnx".
        let file = onnx.first().cloned().unwrap_or_default();
        let lang = file
            .get(0..2)
            .filter(|_| file.as_bytes().get(2) == Some(&b'_'))
            .map(|p| p.to_ascii_lowercase())
            .unwrap_or_else(|| "*".into());
        let gender = piper_gender(&format!("{name} {file}"));
        return Some(base(ModelKind::Tts, Engine::Piper, vec![lang], None, false, gender));
    }
    // Speech recognition: Whisper, transducers, CTC, SenseVoice, Moonshine…
    if let Some(files) = engine::detect(dir) {
        return Some(base(
            ModelKind::Stt,
            Engine::Whisper,
            vec!["*".into()],
            Some(files.family.label().to_string()),
            files.family.is_streaming(),
            "",
        ));
    }
    None
}

/// Finds the first file in `dir` whose name satisfies `pred`.
pub fn find_file(dir: &Path, pred: impl Fn(&str) -> bool) -> Option<PathBuf> {
    let mut names = files_with(dir, pred);
    names.sort();
    names.into_iter().next().map(|n| dir.join(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommends_the_small_starter_set() {
        let rec = recommend(&HardwareInfo::default());
        assert_eq!(rec.stt, "whisper-base");
        let total: u64 = rec.ids().iter().filter_map(|id| catalog::find(id)).map(|m| m.download_size()).sum();
        assert!(total < 400_000_000, "starter download should stay small, got {total}");
        for id in rec.ids() {
            assert!(catalog::find(id).is_some(), "{id}");
        }
    }

    #[test]
    fn detects_manual_models_and_families() {
        let root = tempfile::tempdir().unwrap();
        let store = ModelStore::new(root.path().to_path_buf());

        let w = root.path().join("my-whisper");
        std::fs::create_dir_all(&w).unwrap();
        for f in ["medium-encoder.onnx", "medium-decoder.onnx", "medium-tokens.txt"] {
            std::fs::write(w.join(f), b"x").unwrap();
        }
        let p = root.path().join("vits-piper-de_DE-eva_k-x_low");
        std::fs::create_dir_all(p.join("espeak-ng-data")).unwrap();
        std::fs::write(p.join("de_DE-eva_k-x_low.onnx"), b"x").unwrap();
        std::fs::write(p.join("tokens.txt"), b"x").unwrap();

        // An incomplete catalog download is not reported.
        std::fs::create_dir_all(root.path().join("whisper-small")).unwrap();

        let installed = store.installed();
        assert_eq!(installed.len(), 2, "{installed:?}");
        let piper = installed.iter().find(|m| m.engine == Engine::Piper).unwrap();
        assert_eq!(piper.languages, vec!["de"]);
        assert_eq!(piper.gender, "female");
        let whisper = installed.iter().find(|m| m.kind == ModelKind::Stt).unwrap();
        assert_eq!(whisper.family.as_deref(), Some("Whisper"));
        assert!(!whisper.streaming);
        assert!(whisper.size_bytes > 0);

        std::fs::write(root.path().join("whisper-small").join(COMPLETE_MARKER), b"{}").unwrap();
        assert!(store.is_installed("whisper-small"));
        assert!(store.delete("../evil").is_err());
        store.delete("whisper-small").unwrap();
        assert!(!store.is_installed("whisper-small"));
    }

    #[test]
    fn finds_models_in_hugging_face_cache_layout_and_flags_unusable_ones() {
        let app_dir = tempfile::tempdir().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let store = ModelStore::new(app_dir.path().to_path_buf());

        // A usable ONNX voice inside a Hugging Face cache snapshot folder.
        let snap = cache.path().join("models--k2-fsa--kitten-nano").join("snapshots").join("abc123");
        std::fs::create_dir_all(snap.join("espeak-ng-data")).unwrap();
        for f in ["model.int8.onnx", "voices.bin", "tokens.txt"] {
            std::fs::write(snap.join(f), b"x").unwrap();
        }
        // PyTorch-only models cannot run in the local ONNX engine.
        let torch = cache.path().join("models--Qwen--Qwen3-TTS-12Hz-0.6B-Base").join("snapshots").join("def456");
        std::fs::create_dir_all(&torch).unwrap();
        for f in ["config.json", "model.safetensors"] {
            std::fs::write(torch.join(f), b"x").unwrap();
        }

        store.set_extra_dirs(vec![cache.path().to_path_buf()]);
        let installed = store.installed();
        assert_eq!(installed.len(), 1, "{installed:?}");
        assert_eq!(installed[0].engine, Engine::Kitten);
        assert_eq!(installed[0].kind, ModelKind::Tts);
        assert_eq!(installed[0].name, "k2-fsa/kitten-nano", "Hugging Face folders show the repo name");

        let bad = store.incompatible();
        assert_eq!(bad.len(), 1, "{bad:?}");
        assert_eq!(bad[0].name, "Qwen/Qwen3-TTS-12Hz-0.6B-Base");
        assert!(bad[0].reason.contains("ONNX"));
    }

    #[test]
    fn finds_models_in_extra_folders_including_streaming_ones() {
        let app_dir = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let store = ModelStore::new(app_dir.path().to_path_buf());

        // Mirrors the user's layout: <extra>/parakeet-models/<model>/
        let nested = other.path().join("parakeet-models").join("nemotron-3.5-asr-streaming-0.6b");
        std::fs::create_dir_all(&nested).unwrap();
        for f in ["encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"] {
            std::fs::write(nested.join(f), b"x").unwrap();
        }
        assert!(store.installed().is_empty());

        store.set_extra_dirs(vec![other.path().to_path_buf()]);
        let installed = store.installed();
        assert_eq!(installed.len(), 1, "{installed:?}");
        assert_eq!(installed[0].id, "nemotron-3.5-asr-streaming-0.6b");
        assert!(installed[0].streaming);
        assert_eq!(installed[0].family.as_deref(), Some("Streaming transducer"));
        assert_eq!(installed[0].path, nested);
        // Extra folders are never deleted by the app.
        assert!(store.delete("nemotron-3.5-asr-streaming-0.6b").is_ok());
        assert!(nested.exists());
    }
}
