//! Voice discovery and automatic per-language voice selection.

use crate::services::language::Lang;
use crate::services::models::catalog::{self, Engine, ModelKind};
use crate::services::models::InstalledModel;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInfo {
    /// "<model_id>:<speaker_id>"
    pub id: String,
    pub model_id: String,
    pub name: String,
    pub language: String,
    pub speaker_id: i32,
    pub engine: Engine,
    pub quality: u8,
}

/// Speakers of kokoro-en-v0_19 in speaker-id order.
const KOKORO_V019_SPEAKERS: &[&str] = &[
    "af", "af_bella", "af_nicole", "af_sarah", "af_sky", "am_adam", "am_michael", "bf_emma", "bf_isabella", "bm_george", "bm_lewis",
];
/// Preferred default English speaker (warm, clear American English).
const KOKORO_DEFAULT: &str = "af_bella";

fn pretty_kokoro(name: &str) -> String {
    let (prefix, who) = name.split_once('_').unwrap_or((name, "default"));
    let accent = match prefix.chars().next() {
        Some('a') => "US",
        Some('b') => "UK",
        _ => "",
    };
    let gender = match prefix.chars().nth(1) {
        Some('f') => "female",
        Some('m') => "male",
        _ => "",
    };
    let mut w = who.to_string();
    if let Some(f) = w.get_mut(0..1) {
        f.make_ascii_uppercase();
    }
    format!("{w} ({accent} English, {gender})")
}

pub fn list_voices(installed: &[InstalledModel]) -> Vec<VoiceInfo> {
    let mut out = Vec::new();
    for m in installed.iter().filter(|m| m.kind == ModelKind::Tts) {
        let quality = catalog::find(&m.id).map(|c| c.quality).unwrap_or(3);
        match m.engine {
            Engine::Kokoro => {
                for (sid, speaker) in KOKORO_V019_SPEAKERS.iter().enumerate() {
                    out.push(VoiceInfo {
                        id: format!("{}:{sid}", m.id),
                        model_id: m.id.clone(),
                        name: pretty_kokoro(speaker),
                        language: "en".into(),
                        speaker_id: sid as i32,
                        engine: m.engine,
                        // Rank the preferred default speaker first.
                        quality: if *speaker == KOKORO_DEFAULT { quality + 1 } else { quality },
                    });
                }
            }
            Engine::Piper => {
                let lang = m.languages.first().cloned().unwrap_or_else(|| "*".into());
                out.push(VoiceInfo {
                    id: format!("{}:0", m.id),
                    model_id: m.id.clone(),
                    name: m.name.clone(),
                    language: lang,
                    speaker_id: 0,
                    engine: m.engine,
                    quality,
                });
            }
            _ => {}
        }
    }
    out
}

/// `preference` is "auto" or a voice id from settings.
pub fn select_voice(voices: &[VoiceInfo], lang: Lang, preference: &str) -> Option<VoiceInfo> {
    if preference != "auto" {
        if let Some(v) = voices.iter().find(|v| v.id == preference && v.language == lang.code()) {
            return Some(v.clone());
        }
    }
    voices
        .iter()
        .filter(|v| v.language == lang.code())
        .max_by(|a, b| a.quality.cmp(&b.quality).then_with(|| b.id.cmp(&a.id)))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn installed(id: &str, engine: Engine, lang: &str) -> InstalledModel {
        InstalledModel {
            id: id.into(),
            name: id.into(),
            kind: ModelKind::Tts,
            engine,
            languages: vec![lang.into()],
            path: PathBuf::new(),
            source: "catalog".into(),
        }
    }

    #[test]
    fn automatic_selection_per_language() {
        let voices = list_voices(&[
            installed("kokoro-en-v0_19", Engine::Kokoro, "en"),
            installed("piper-de_DE-thorsten-high", Engine::Piper, "de"),
            installed("piper-de_DE-thorsten-medium-int8", Engine::Piper, "de"),
            installed("piper-ar_JO-kareem-medium", Engine::Piper, "ar"),
        ]);
        assert_eq!(select_voice(&voices, Lang::En, "auto").unwrap().speaker_id, 1); // af_bella
        assert_eq!(select_voice(&voices, Lang::De, "auto").unwrap().model_id, "piper-de_DE-thorsten-high");
        assert_eq!(select_voice(&voices, Lang::Ar, "auto").unwrap().language, "ar");
        // Manual override
        assert_eq!(select_voice(&voices, Lang::En, "kokoro-en-v0_19:9").unwrap().speaker_id, 9);
        // An override for another language is ignored (prevents wrong-language voice)
        assert_eq!(select_voice(&voices, Lang::De, "kokoro-en-v0_19:9").unwrap().language, "de");
    }

    #[test]
    fn unavailable_voice() {
        let voices = list_voices(&[installed("kokoro-en-v0_19", Engine::Kokoro, "en")]);
        assert!(select_voice(&voices, Lang::Ar, "auto").is_none());
        assert!(select_voice(&[], Lang::En, "auto").is_none());
    }

    #[test]
    fn kokoro_names() {
        assert_eq!(pretty_kokoro("bm_george"), "George (UK English, male)");
    }
}
