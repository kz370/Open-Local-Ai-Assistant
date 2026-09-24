//! Curated catalog of local voice models (sherpa-onnx exports).
//!
//! URLs are pinned to specific release assets and verified with SHA-256
//! (where the publisher exposes a digest) before installation. Nothing is
//! downloaded without the user's explicit consent.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    Stt,
    Vad,
    Tts,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Whisper,
    SileroVad,
    Kokoro,
    Piper,
    Kitten,
    /// Supertonic (multilingual flow-matching voices, sherpa-onnx).
    Supertonic,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFile {
    pub url: &'static str,
    pub sha256: Option<&'static str>,
    /// Destination path relative to the model directory; for archives, the
    /// archive is extracted into the model directory (top folder stripped).
    pub dest: &'static str,
    pub archive: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: &'static str,
    pub kind: ModelKind,
    pub engine: Engine,
    pub name: &'static str,
    /// Supported languages ("*" = multilingual incl. en/ar/de).
    pub languages: &'static [&'static str],
    pub files: &'static [RemoteFile],
    /// Relative quality 1..5 used by automatic selection.
    pub quality: u8,
    /// Suggested minimum RAM in GB for comfortable CPU use.
    pub min_ram_gb: u32,
    pub license: &'static str,
    /// "female" | "male" | "mixed" (voice models only).
    pub gender: &'static str,
}

impl CatalogModel {
    pub fn download_size(&self) -> u64 {
        self.files.iter().map(|f| f.size_bytes).sum()
    }
}

const HF: &str = "https://huggingface.co/csukuangfj";
const GH_ASR: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models";
const GH_TTS: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models";

macro_rules! url {
    ($base:expr, $path:literal) => {
        const_format_concat!($base, $path)
    };
}

// Tiny const concat helper (avoids a dependency).
macro_rules! const_format_concat {
    ($a:expr, $b:literal) => {{
        const A: &str = $a;
        const B: &str = $b;
        const LEN: usize = A.len() + B.len();
        const BYTES: [u8; LEN] = {
            let mut out = [0u8; LEN];
            let (a, b) = (A.as_bytes(), B.as_bytes());
            let mut i = 0;
            while i < a.len() {
                out[i] = a[i];
                i += 1;
            }
            let mut j = 0;
            while j < b.len() {
                out[a.len() + j] = b[j];
                j += 1;
            }
            out
        };
        // SAFETY: concatenation of two valid UTF-8 strings is valid UTF-8.
        unsafe { std::str::from_utf8_unchecked(&BYTES) }
    }};
}

const WHISPER_TOKENS_SHA: &str = "b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126";

pub static CATALOG: &[CatalogModel] = &[
    // ---------------- Speech recognition ----------------
    CatalogModel {
        id: "whisper-turbo",
        kind: ModelKind::Stt,
        engine: Engine::Whisper,
        name: "Whisper large-v3 turbo (int8)",
        languages: &["*"],
        files: &[
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-turbo/resolve/main/turbo-encoder.int8.onnx"), sha256: Some("b02dcdf54f348741e93fe732b67d933c8dcb6735655f710640143081db38878b"), dest: "encoder.int8.onnx", archive: false, size_bytes: 674_700_000 },
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-turbo/resolve/main/turbo-decoder.int8.onnx"), sha256: Some("20accd02388482eb3a46bd615631adfdc85e1eb2c7db9ea3f02a40ffe6b81547"), dest: "decoder.int8.onnx", archive: false, size_bytes: 361_100_000 },
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-turbo/resolve/main/turbo-tokens.txt"), sha256: Some(WHISPER_TOKENS_SHA), dest: "tokens.txt", archive: false, size_bytes: 816_730 },
        ],
        quality: 5,
        min_ram_gb: 12,
        license: "MIT",
        gender: "",
    },
    CatalogModel {
        id: "whisper-tiny",
        kind: ModelKind::Stt,
        engine: Engine::Whisper,
        name: "Whisper tiny (int8) — smallest",
        languages: &["*"],
        files: &[
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-tiny/resolve/main/tiny-encoder.int8.onnx"), sha256: Some("d24fb083ae3b1041fc24e97971d60e280c9342201fbb67b0ab428a8b4a51a434"), dest: "encoder.int8.onnx", archive: false, size_bytes: 12_900_000 },
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-tiny/resolve/main/tiny-decoder.int8.onnx"), sha256: Some("d2fece8dd42771f1df975c6c0445770d0c292bf7547c2cae04a6c0cc57540925"), dest: "decoder.int8.onnx", archive: false, size_bytes: 89_900_000 },
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-tiny/resolve/main/tiny-tokens.txt"), sha256: Some(WHISPER_TOKENS_SHA), dest: "tokens.txt", archive: false, size_bytes: 816_730 },
        ],
        quality: 1,
        min_ram_gb: 1,
        license: "MIT",
        gender: "",
    },
    CatalogModel {
        id: "whisper-small",
        kind: ModelKind::Stt,
        engine: Engine::Whisper,
        name: "Whisper small (int8)",
        languages: &["*"],
        files: &[
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-small/resolve/main/small-encoder.int8.onnx"), sha256: Some("4cbe7b22fa9026b843b60a68640c747de05bafb1a11b57edc0e66c232d9f33a9"), dest: "encoder.int8.onnx", archive: false, size_bytes: 112_400_000 },
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-small/resolve/main/small-decoder.int8.onnx"), sha256: Some("acad50b5c782696e91b55914cc5ab4f756f1532f76e22aa6fc615f39fb69a8ee"), dest: "decoder.int8.onnx", archive: false, size_bytes: 262_200_000 },
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-small/resolve/main/small-tokens.txt"), sha256: Some(WHISPER_TOKENS_SHA), dest: "tokens.txt", archive: false, size_bytes: 816_730 },
        ],
        quality: 3,
        min_ram_gb: 4,
        license: "MIT",
        gender: "",
    },
    CatalogModel {
        id: "whisper-base",
        kind: ModelKind::Stt,
        engine: Engine::Whisper,
        name: "Whisper base (int8)",
        languages: &["*"],
        files: &[
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-base/resolve/main/base-encoder.int8.onnx"), sha256: Some("0b8fb1304b6109976038efff5ace81720e00386f3ff6b54ee8c75291ca0a1e11"), dest: "encoder.int8.onnx", archive: false, size_bytes: 29_100_000 },
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-base/resolve/main/base-decoder.int8.onnx"), sha256: Some("9759d217388a01b3a4c7c15533201067b48ae819c4daafc8624e64b9409dc02d"), dest: "decoder.int8.onnx", archive: false, size_bytes: 130_700_000 },
            RemoteFile { url: url!(HF, "/sherpa-onnx-whisper-base/resolve/main/base-tokens.txt"), sha256: None, dest: "tokens.txt", archive: false, size_bytes: 816_730 },
        ],
        quality: 2,
        min_ram_gb: 2,
        license: "MIT",
        gender: "",
    },
    // ---------------- Voice activity detection ----------------
    CatalogModel {
        id: "silero-vad",
        kind: ModelKind::Vad,
        engine: Engine::SileroVad,
        name: "Silero VAD",
        languages: &["*"],
        files: &[RemoteFile { url: url!(GH_ASR, "/silero_vad.onnx"), sha256: Some("9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"), dest: "silero_vad.onnx", archive: false, size_bytes: 629_000 }],
        quality: 5,
        min_ram_gb: 1,
        license: "MIT",
        gender: "",
    },
    // ---------------- Text-to-speech ----------------
    CatalogModel {
        id: "supertonic-3-int8",
        kind: ModelKind::Tts,
        engine: Engine::Supertonic,
        name: "Supertonic 3 multilingual - small (31 languages)",
        languages: &["*"],
        files: &[RemoteFile { url: url!(GH_TTS, "/sherpa-onnx-supertonic-3-tts-int8-2026-05-11.tar.bz2"), sha256: Some("82fa96f91c4ef8abaae3a14a3f4153facf88bed821d1f7331cec2700f432c427"), dest: "", archive: true, size_bytes: 128_774_318 }],
        quality: 3,
        min_ram_gb: 2,
        license: "OpenRAIL-M",
        gender: "mixed",
    },
];

pub fn find(id: &str) -> Option<&'static CatalogModel> {
    CATALOG.iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_consistent() {
        let mut ids = std::collections::HashSet::new();
        for m in CATALOG {
            assert!(ids.insert(m.id), "duplicate id {}", m.id);
            assert!(!m.files.is_empty());
            for f in m.files {
                assert!(f.url.starts_with("https://"), "{}", f.url);
                if let Some(h) = f.sha256 {
                    assert_eq!(h.len(), 64);
                }
            }
        }
        for lang in ["en", "ar", "de"] {
            assert!(CATALOG.iter().any(|m| m.kind == ModelKind::Tts && (m.languages.contains(&lang) || m.languages.contains(&"*"))), "no TTS voice for {lang}");
        }
        assert!(find("silero-vad").is_some());
        assert_eq!(find("whisper-small").unwrap().files[0].url, "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-small/resolve/main/small-encoder.int8.onnx");
    }
}
