//! Local voice model management: discovery of installed models (catalog and
//! manually placed), hardware-based recommendations and consented downloads.

pub mod catalog;
pub mod download;

use crate::services::hardware::HardwareInfo;
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

pub fn recommend(hw: &HardwareInfo) -> Recommendation {
    const GB: u64 = 1024 * 1024 * 1024;
    let ram = hw.total_ram_bytes;
    let strong = ram >= 12 * GB && hw.physical_cores >= 6;
    let medium = ram >= 6 * GB && hw.physical_cores >= 4;
    Recommendation {
        stt: if strong { "whisper-turbo" } else if medium { "whisper-small" } else { "whisper-base" },
        vad: "silero-vad",
        tts_en: if medium { "kokoro-en-v0_19" } else { "kokoro-int8-en-v0_19" },
        tts_de: if medium { "piper-de_DE-thorsten-high" } else { "piper-de_DE-thorsten-medium-int8" },
        tts_ar: "piper-ar_JO-kareem-medium",
    }
}

pub struct ModelStore {
    pub dir: PathBuf,
}

impl ModelStore {
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self { dir }
    }

    pub fn model_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.model_dir(id).join(COMPLETE_MARKER).exists()
    }

    /// Lists catalog models that finished installing plus compatible models the
    /// user copied into the models folder manually.
    pub fn installed(&self) -> Vec<InstalledModel> {
        let mut out: Vec<InstalledModel> = CATALOG
            .iter()
            .filter(|m| self.is_installed(m.id))
            .map(|m| from_catalog(m, self.model_dir(m.id)))
            .collect();
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return out };
        for e in entries.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if !path.is_dir() || catalog::find(&name).is_some() || name.starts_with('.') {
                continue;
            }
            if let Some(m) = detect_custom(&path, &name) {
                out.push(m);
            }
        }
        out
    }

    pub fn find_installed(&self, id: &str) -> Option<InstalledModel> {
        self.installed().into_iter().find(|m| m.id == id)
    }

    pub fn delete(&self, id: &str) -> std::io::Result<()> {
        let dir = self.model_dir(id);
        // Guard against path traversal through crafted ids.
        if id.contains(['/', '\\']) || id.contains("..") || !dir.starts_with(&self.dir) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid model id"));
        }
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }
}

fn from_catalog(m: &CatalogModel, path: PathBuf) -> InstalledModel {
    InstalledModel {
        id: m.id.into(),
        name: m.name.into(),
        kind: m.kind,
        engine: m.engine,
        languages: m.languages.iter().map(|s| s.to_string()).collect(),
        path,
        source: "catalog".into(),
    }
}

fn files_with(dir: &Path, pred: impl Fn(&str) -> bool) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|it| it.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| pred(n)).collect())
        .unwrap_or_default()
}

/// Recognizes manually installed sherpa-onnx model folders.
pub fn detect_custom(dir: &Path, name: &str) -> Option<InstalledModel> {
    let has = |f: &str| dir.join(f).exists();
    let onnx = files_with(dir, |n| n.ends_with(".onnx"));
    let tokens = files_with(dir, |n| n.ends_with("tokens.txt"));
    let base = |kind, engine, langs: Vec<String>| InstalledModel {
        id: name.to_string(),
        name: name.to_string(),
        kind,
        engine,
        languages: langs,
        path: dir.to_path_buf(),
        source: "custom".into(),
    };
    if onnx.iter().any(|n| n.contains("encoder")) && onnx.iter().any(|n| n.contains("decoder")) && !tokens.is_empty() {
        return Some(base(ModelKind::Stt, Engine::Whisper, vec!["*".into()]));
    }
    if onnx.iter().any(|n| n.starts_with("silero_vad")) {
        return Some(base(ModelKind::Vad, Engine::SileroVad, vec!["*".into()]));
    }
    if has("voices.bin") && !onnx.is_empty() && !tokens.is_empty() {
        return Some(base(ModelKind::Tts, Engine::Kokoro, vec!["en".into()]));
    }
    if !onnx.is_empty() && !tokens.is_empty() && has("espeak-ng-data") {
        // Piper voices are named like "de_DE-thorsten-high.onnx".
        let lang = onnx
            .iter()
            .find_map(|n| n.get(0..2).filter(|_| n.as_bytes().get(2) == Some(&b'_')).map(|p| p.to_ascii_lowercase()))
            .unwrap_or_else(|| "*".into());
        return Some(base(ModelKind::Tts, Engine::Piper, vec![lang]));
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
    use crate::services::hardware::GpuInfo;

    #[test]
    fn recommendations_scale_with_hardware() {
        const GB: u64 = 1024 * 1024 * 1024;
        let strong = HardwareInfo { total_ram_bytes: 32 * GB, physical_cores: 8, gpus: vec![GpuInfo::default()], ..Default::default() };
        let weak = HardwareInfo { total_ram_bytes: 4 * GB, physical_cores: 2, ..Default::default() };
        assert_eq!(recommend(&strong).stt, "whisper-turbo");
        assert_eq!(recommend(&weak).stt, "whisper-base");
        assert_eq!(recommend(&weak).tts_en, "kokoro-int8-en-v0_19");
        for id in recommend(&weak).ids().into_iter().chain(recommend(&strong).ids()) {
            assert!(catalog::find(id).is_some(), "{id}");
        }
    }

    #[test]
    fn detects_manual_models() {
        let root = tempfile::tempdir().unwrap();
        let store = ModelStore::new(root.path().to_path_buf());

        let w = root.path().join("my-whisper");
        std::fs::create_dir_all(&w).unwrap();
        for f in ["medium-encoder.onnx", "medium-decoder.onnx", "medium-tokens.txt"] {
            std::fs::write(w.join(f), b"x").unwrap();
        }
        let p = root.path().join("vits-piper-ar_JO-test");
        std::fs::create_dir_all(p.join("espeak-ng-data")).unwrap();
        std::fs::write(p.join("ar_JO-test-medium.onnx"), b"x").unwrap();
        std::fs::write(p.join("tokens.txt"), b"x").unwrap();

        // An incomplete catalog download is not reported.
        std::fs::create_dir_all(root.path().join("whisper-small")).unwrap();

        let installed = store.installed();
        assert_eq!(installed.len(), 2);
        let piper = installed.iter().find(|m| m.engine == Engine::Piper).unwrap();
        assert_eq!(piper.languages, vec!["ar"]);
        assert!(installed.iter().any(|m| m.engine == Engine::Whisper && m.source == "custom"));

        std::fs::write(root.path().join("whisper-small").join(COMPLETE_MARKER), b"{}").unwrap();
        assert!(store.is_installed("whisper-small"));
        assert!(store.delete("../evil").is_err());
        store.delete("whisper-small").unwrap();
        assert!(!store.is_installed("whisper-small"));
    }
}
