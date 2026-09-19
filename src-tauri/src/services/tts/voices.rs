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
    /// "female" | "male" | "" when unknown.
    pub gender: String,
}

/// Speakers of kokoro-en-v0_19 in speaker-id order.
const KOKORO_V019_SPEAKERS: &[&str] = &[
    "af", "af_bella", "af_nicole", "af_sarah", "af_sky", "am_adam", "am_michael", "bf_emma", "bf_isabella", "bm_george", "bm_lewis",
];
/// Preferred default English speaker (warm, clear American English).
const KOKORO_DEFAULT: &str = "af_bella";

fn kokoro_gender(name: &str) -> &'static str {
    match name.split('_').next().and_then(|p| p.chars().nth(1)) {
        Some('m') => "male",
        _ => "female",
    }
}

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
            // Kokoro-architecture models for other languages (e.g. Nabra for
            // Arabic) expose numbered style vectors instead of named speakers.
            Engine::Kokoro if m.languages.first().map(|l| l != "en").unwrap_or(false) => {
                let lang = m.languages.first().cloned().unwrap_or_else(|| "*".into());
                for sid in 0..8 {
                    out.push(VoiceInfo {
                        id: format!("{}:{sid}", m.id),
                        model_id: m.id.clone(),
                        name: format!("{} - voice {}", m.name, sid + 1),
                        language: lang.clone(),
                        speaker_id: sid,
                        engine: m.engine,
                        quality: if sid == 0 { quality + 1 } else { quality },
                        gender: String::new(),
                    });
                }
            }
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
                        gender: kokoro_gender(speaker).into(),
                    });
                }
            }
            Engine::Kitten => {
                // Kitten models ship several speakers in one file.
                for sid in 0..8 {
                    out.push(VoiceInfo {
                        id: format!("{}:{sid}", m.id),
                        model_id: m.id.clone(),
                        name: format!("{} - voice {}", m.name, sid + 1),
                        language: "en".into(),
                        speaker_id: sid,
                        engine: m.engine,
                        quality: if sid == 0 { quality + 1 } else { quality },
                        gender: if sid % 2 == 0 { "female".into() } else { "male".into() },
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
                    gender: if m.gender.is_empty() { catalog::find(&m.id).map(|c| c.gender.to_string()).unwrap_or_default() } else { m.gender.clone() },
                });
            }
            _ => {}
        }
    }
    out
}

pub const SILMA_VOICE_ID: &str = "silma:0";
pub const SILMA_EN_VOICE_ID: &str = "silma:en";

/// The SILMA voices (see services::silma). SILMA is bilingual: for Arabic it
/// ranks above every ONNX voice; for English it is offered but not picked
/// automatically (it speaks with the Arabic reference speaker's voice).
pub fn silma_voices() -> Vec<VoiceInfo> {
    vec![
        VoiceInfo {
            id: SILMA_VOICE_ID.into(),
            model_id: "silma".into(),
            name: "SILMA (natural Arabic)".into(),
            language: "ar".into(),
            speaker_id: 0,
            engine: Engine::Silma,
            quality: 9,
            gender: String::new(),
        },
        VoiceInfo {
            id: SILMA_EN_VOICE_ID.into(),
            model_id: "silma".into(),
            name: "SILMA (English)".into(),
            language: "en".into(),
            speaker_id: 0,
            engine: Engine::Silma,
            quality: 1,
            gender: String::new(),
        },
    ]
}

/// Orpheus voices for every language that has an LM Studio model set. They
/// rank above every local voice, so a configured language speaks with Orpheus.
pub fn orpheus_voices(models: &std::collections::BTreeMap<String, String>) -> Vec<VoiceInfo> {
    let mut out = Vec::new();
    for (lang, model) in models.iter().filter(|(_, m)| !m.trim().is_empty()) {
        for (sid, (name, gender)) in super::orpheus::voices_for(lang).iter().enumerate() {
            let mut pretty = name.to_string();
            pretty[..1].make_ascii_uppercase();
            out.push(VoiceInfo {
                id: format!("orpheus-{lang}:{name}"),
                model_id: model.clone(),
                name: format!("{pretty} (Orpheus)"),
                language: lang.clone(),
                speaker_id: sid as i32,
                engine: Engine::Orpheus,
                quality: if sid == 0 { 21 } else { 20 },
                gender: gender.to_string(),
            });
        }
    }
    out
}

/// `preference` is "auto" or a voice id from settings; `gender` is
/// "any" | "female" | "male" and only steers the automatic choice.
pub fn select_voice(voices: &[VoiceInfo], lang: Lang, preference: &str, gender: &str) -> Option<VoiceInfo> {
    if preference != "auto" {
        if let Some(v) = voices.iter().find(|v| v.id == preference && v.language == lang.code()) {
            return Some(v.clone());
        }
    }
    let best = |filter: &dyn Fn(&VoiceInfo) -> bool| {
        voices
            .iter()
            .filter(|v| v.language == lang.code() && filter(v))
            .max_by(|a, b| a.quality.cmp(&b.quality).then_with(|| b.id.cmp(&a.id)))
            .cloned()
    };
    if gender == "female" || gender == "male" {
        if let Some(v) = best(&|v: &VoiceInfo| v.gender == gender) {
            return Some(v);
        }
    }
    best(&|_| true)
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
            family: None,
            streaming: false,
            gender: String::new(),
            size_bytes: 0,
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
        assert_eq!(select_voice(&voices, Lang::En, "auto", "any").unwrap().speaker_id, 1); // af_bella
        assert_eq!(select_voice(&voices, Lang::De, "auto", "any").unwrap().model_id, "piper-de_DE-thorsten-high");
        assert_eq!(select_voice(&voices, Lang::Ar, "auto", "any").unwrap().language, "ar");
        // Manual override
        assert_eq!(select_voice(&voices, Lang::En, "kokoro-en-v0_19:9", "any").unwrap().speaker_id, 9);
        // An override for another language is ignored (prevents wrong-language voice)
        assert_eq!(select_voice(&voices, Lang::De, "kokoro-en-v0_19:9", "any").unwrap().language, "de");
        // A gender preference steers the automatic choice
        assert_eq!(select_voice(&voices, Lang::En, "auto", "male").unwrap().gender, "male");
        assert_eq!(select_voice(&voices, Lang::En, "auto", "female").unwrap().gender, "female");
        // ...and falls back when that language has no such voice
        assert_eq!(select_voice(&voices, Lang::De, "auto", "female").unwrap().language, "de");
    }

    #[test]
    fn unavailable_voice() {
        let voices = list_voices(&[installed("kokoro-en-v0_19", Engine::Kokoro, "en")]);
        assert!(select_voice(&voices, Lang::Ar, "auto", "any").is_none());
        assert!(select_voice(&[], Lang::En, "auto", "any").is_none());
    }

    #[test]
    fn kokoro_names() {
        assert_eq!(pretty_kokoro("bm_george"), "George (UK English, male)");
    }
}
