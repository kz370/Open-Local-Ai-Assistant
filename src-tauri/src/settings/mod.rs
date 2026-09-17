//! Persistent application settings (stored as one JSON document in SQLite).
//!
//! Every field has a serde default so settings written by older versions keep
//! loading after new fields are added.

use crate::database::Db;
use crate::errors::AppResult;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

const KEY: &str = "app_settings";
pub const DEFAULT_LMSTUDIO_URL: &str = "http://localhost:1234/v1";
const CURRENT_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self { x: 0, y: 0, width: 420, height: 640 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneralSettings {
    /// "system" | "light" | "dark"
    pub theme: String,
    pub high_contrast: bool,
    pub font_scale: f32,
    pub start_with_os: bool,
    pub start_minimized: bool,
    pub always_on_top: bool,
    /// "bottom-right" | "bottom-left" | "center" | "custom"
    pub window_position: String,
    pub window: WindowGeometry,
    pub compact: bool,
    pub global_shortcut: String,
    pub push_to_talk_shortcut: String,
    pub developer_mode: bool,
    pub first_run_complete: bool,
    pub log_conversation_content: bool,
    /// Saved position of the floating bubble (physical pixels).
    pub bubble_x: Option<i32>,
    pub bubble_y: Option<i32>,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            high_contrast: false,
            font_scale: 1.0,
            start_with_os: false,
            start_minimized: false,
            always_on_top: true,
            window_position: "bottom-right".into(),
            window: WindowGeometry::default(),
            compact: false,
            global_shortcut: "CommandOrControl+Space".into(),
            push_to_talk_shortcut: "CommandOrControl+Shift+Space".into(),
            developer_mode: false,
            first_run_complete: false,
            log_conversation_content: false,
            bubble_x: None,
            bubble_y: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AiSettings {
    pub server_url: String,
    /// "auto" | "manual"
    pub model_mode: String,
    pub model: Option<String>,
    pub temperature: f32,
    pub context_length: Option<u32>,
    pub max_tokens: Option<u32>,
    pub system_prompt: String,
    pub streaming: bool,
    pub request_timeout_secs: u64,
    pub show_reasoning: bool,
    /// User-defined short display names, keyed by LM Studio model id.
    pub model_aliases: std::collections::BTreeMap<String, String>,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            server_url: DEFAULT_LMSTUDIO_URL.into(),
            model_mode: "auto".into(),
            model: None,
            temperature: 0.7,
            context_length: None,
            max_tokens: None,
            system_prompt: String::new(),
            streaming: true,
            request_timeout_secs: 300,
            show_reasoning: false,
            model_aliases: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LanguageSettings {
    /// "auto" | "en" | "ar" | "de"
    pub response_language: String,
}

impl Default for LanguageSettings {
    fn default() -> Self {
        Self { response_language: "auto".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SttSettings {
    /// "auto" or a catalog model id
    pub model: String,
    /// "auto" | "en" | "ar" | "de"
    pub language: String,
    /// None = automatic (system default input)
    pub microphone: Option<String>,
    /// "auto" | "cpu"
    pub hardware: String,
    pub auto_submit: bool,
    pub hands_free: bool,
    pub push_to_talk: bool,
    pub vad_threshold: f32,
    pub silence_ms: u32,
    /// Extra folders scanned for speech/voice models (read-only).
    pub extra_model_dirs: Vec<String>,
}

impl Default for SttSettings {
    fn default() -> Self {
        Self {
            model: "auto".into(),
            language: "auto".into(),
            microphone: None,
            hardware: "auto".into(),
            auto_submit: true,
            hands_free: false,
            push_to_talk: true,
            vad_threshold: 0.5,
            silence_ms: 800,
            extra_model_dirs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct TtsSettings {
    pub speak_responses: bool,
    /// "auto" or a voice id, per language
    pub voice_en: String,
    pub voice_ar: String,
    pub voice_de: String,
    pub speed: f32,
    pub volume: f32,
    /// None = automatic (system default output)
    pub output_device: Option<String>,
    /// "any" | "female" | "male" - used when a voice is chosen automatically.
    pub preferred_gender: String,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            speak_responses: false,
            voice_en: "auto".into(),
            voice_ar: "auto".into(),
            voice_de: "auto".into(),
            speed: 1.0,
            volume: 1.0,
            output_device: None,
            preferred_gender: "any".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DictationSettings {
    pub enabled: bool,
    pub shortcut: String,
    /// "hold" (push-to-talk) | "toggle"
    pub mode: String,
    pub correction_enabled: bool,
    /// LM Studio model id chosen by the user for text correction.
    pub correction_model: Option<String>,
    /// "type" | "paste"
    pub insert_method: String,
    pub add_trailing_space: bool,
}

impl Default for DictationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            shortcut: "CommandOrControl+Alt+Space".into(),
            mode: "hold".into(),
            correction_enabled: false,
            correction_model: None,
            insert_method: "type".into(),
            add_trailing_space: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub general: GeneralSettings,
    pub ai: AiSettings,
    pub language: LanguageSettings,
    pub stt: SttSettings,
    pub tts: TtsSettings,
    pub dictation: DictationSettings,
    pub last_conversation_id: Option<String>,
    /// Schema version of the stored settings, used for one-time migrations.
    pub version: u32,
}

impl Settings {
    /// Clamp values into sane ranges; applied on every save.
    pub fn sanitize(&mut self) {
        let lang_ok = |s: &str| matches!(s, "auto" | "en" | "ar" | "de");
        if !lang_ok(&self.language.response_language) {
            self.language.response_language = "auto".into();
        }
        if !lang_ok(&self.stt.language) {
            self.stt.language = "auto".into();
        }
        if !matches!(self.general.theme.as_str(), "system" | "light" | "dark") {
            self.general.theme = "system".into();
        }
        if !matches!(self.general.window_position.as_str(), "bottom-right" | "bottom-left" | "center" | "custom") {
            self.general.window_position = "bottom-right".into();
        }
        self.general.font_scale = self.general.font_scale.clamp(0.8, 1.6);
        self.general.window.width = self.general.window.width.clamp(320, 4000);
        self.general.window.height = self.general.window.height.clamp(260, 4000);
        self.ai.temperature = self.ai.temperature.clamp(0.0, 2.0);
        self.ai.server_url = self.ai.server_url.trim().trim_end_matches('/').to_string();
        if self.ai.server_url.is_empty() {
            self.ai.server_url = DEFAULT_LMSTUDIO_URL.into();
        }
        if !matches!(self.ai.model_mode.as_str(), "auto" | "manual") {
            self.ai.model_mode = "auto".into();
        }
        self.ai.request_timeout_secs = self.ai.request_timeout_secs.clamp(10, 3600);
        self.ai.model_aliases = std::mem::take(&mut self.ai.model_aliases)
            .into_iter()
            .map(|(k, v)| (k, v.trim().chars().take(40).collect::<String>()))
            .filter(|(k, v)| !k.is_empty() && !v.is_empty())
            .collect();
        self.tts.speed = self.tts.speed.clamp(0.5, 2.0);
        self.tts.volume = self.tts.volume.clamp(0.0, 1.0);
        self.stt.vad_threshold = self.stt.vad_threshold.clamp(0.1, 0.95);
        self.stt.silence_ms = self.stt.silence_ms.clamp(200, 5000);
        self.stt.extra_model_dirs = std::mem::take(&mut self.stt.extra_model_dirs)
            .into_iter()
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty())
            .collect();
        self.stt.extra_model_dirs.dedup();
        if !matches!(self.tts.preferred_gender.as_str(), "any" | "female" | "male") {
            self.tts.preferred_gender = "any".into();
        }
        if !matches!(self.dictation.mode.as_str(), "hold" | "toggle") {
            self.dictation.mode = "hold".into();
        }
        if !matches!(self.dictation.insert_method.as_str(), "type" | "paste") {
            self.dictation.insert_method = "type".into();
        }
    }
}

pub struct SettingsStore {
    db: Arc<Db>,
    current: RwLock<Settings>,
}

impl SettingsStore {
    pub fn load(db: Arc<Db>) -> AppResult<Self> {
        let mut s = match db.get_kv(KEY)? {
            Some(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "settings unreadable, using defaults");
                Settings::default()
            }),
            None => Settings::default(),
        };
        s.sanitize();
        let store = Self { db, current: RwLock::new(s) };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> AppResult<()> {
        let s = self.get();
        if s.version >= CURRENT_VERSION {
            return Ok(());
        }
        self.update(|s| {
            if s.version < 2 {
                // v2: dictation became a default feature.
                s.dictation.enabled = true;
            }
            s.version = CURRENT_VERSION;
        })?;
        Ok(())
    }

    pub fn get(&self) -> Settings {
        self.current.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn set(&self, mut s: Settings) -> AppResult<Settings> {
        s.sanitize();
        self.db.set_kv(KEY, &serde_json::to_string(&s)?)?;
        *self.current.write().unwrap_or_else(|p| p.into_inner()) = s.clone();
        Ok(s)
    }

    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> AppResult<Settings> {
        let mut s = self.get();
        f(&mut s);
        self.set(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_persistence() {
        let db = Arc::new(Db::open_in_memory().unwrap());
        let store = SettingsStore::load(db.clone()).unwrap();
        let s = store.get();
        assert_eq!(s.ai.server_url, DEFAULT_LMSTUDIO_URL);
        assert_eq!(s.ai.model_mode, "auto");
        assert_eq!(s.general.window_position, "bottom-right");
        assert_eq!(s.general.global_shortcut, "CommandOrControl+Space");

        store
            .update(|s| {
                s.ai.server_url = "http://127.0.0.1:4321/v1/".into();
                s.general.theme = "dark".into();
                s.general.window = WindowGeometry { x: 10, y: 20, width: 500, height: 700 };
            })
            .unwrap();
        let reloaded = SettingsStore::load(db).unwrap().get();
        assert_eq!(reloaded.ai.server_url, "http://127.0.0.1:4321/v1");
        assert_eq!(reloaded.general.theme, "dark");
        assert_eq!(reloaded.general.window.x, 10);
    }

    #[test]
    fn sanitize_rejects_bad_values() {
        let mut s = Settings::default();
        s.language.response_language = "fr".into();
        s.ai.temperature = 9.0;
        s.tts.volume = 3.0;
        s.ai.server_url = "  ".into();
        s.sanitize();
        assert_eq!(s.language.response_language, "auto");
        assert_eq!(s.ai.temperature, 2.0);
        assert_eq!(s.tts.volume, 1.0);
        assert_eq!(s.ai.server_url, DEFAULT_LMSTUDIO_URL);
    }

    #[test]
    fn migration_enables_dictation_once() {
        let db = Arc::new(Db::open_in_memory().unwrap());
        db.set_kv(KEY, r#"{"dictation":{"enabled":false}}"#).unwrap();
        let store = SettingsStore::load(db.clone()).unwrap();
        assert!(store.get().dictation.enabled);
        assert_eq!(store.get().version, CURRENT_VERSION);
        // A later explicit opt-out is respected.
        store.update(|s| s.dictation.enabled = false).unwrap();
        assert!(!SettingsStore::load(db).unwrap().get().dictation.enabled);
    }

    #[test]
    fn partial_json_uses_defaults() {
        let s: Settings = serde_json::from_str(r#"{"ai":{"temperature":0.2}}"#).unwrap();
        assert_eq!(s.ai.temperature, 0.2);
        assert_eq!(s.ai.server_url, DEFAULT_LMSTUDIO_URL);
    }
}
