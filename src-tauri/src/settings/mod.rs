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
const CURRENT_VERSION: u32 = 3;

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
        Self { x: 0, y: 0, width: 480, height: 640 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneralSettings {
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "teal" | "blue" | "green" | "amber" | "rose" | "slate"
    pub accent: String,
    /// What the assistant calls itself in the UI and in its system prompt.
    pub assistant_name: String,
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
            accent: "teal".into(),
            assistant_name: "Local Assistant".into(),
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
    /// Bearer token sent as `Authorization: Bearer <key>`. LM Studio itself
    /// ignores it, but other OpenAI-compatible servers behind this same URL
    /// field (OpenRouter, a hosted vLLM, etc.) may require one.
    pub api_key: Option<String>,
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
            api_key: None,
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

fn default_true() -> bool {
    true
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
    /// true = microphone only (loopback / Stereo Mix excluded)
    #[serde(default = "default_true")]
    pub mic_only: bool,
    /// true = ignore microphone audio while the PC's own speakers are
    /// playing (via WASAPI loopback level), so system audio can't bleed
    /// into a real microphone's recording.
    #[serde(default = "default_true")]
    pub isolate_system_audio: bool,
    /// "auto" | "cpu"
    pub hardware: String,
    pub auto_submit: bool,
    pub hands_free: bool,
    pub push_to_talk: bool,
    pub vad_threshold: f32,
    pub silence_ms: u32,
    /// Extra folders scanned for speech/voice models (read-only).
    pub extra_model_dirs: Vec<String>,
    /// Show hands-free conversation as a call screen.
    pub call_view: bool,
    /// Stop a recording after this much silence (seconds, 0 = never).
    pub auto_stop_silence_secs: u32,
    /// Leave hands-free mode after this long without speech (seconds, 0 = never).
    pub hands_free_timeout_secs: u32,
}

impl Default for SttSettings {
    fn default() -> Self {
        Self {
            model: "auto".into(),
            language: "auto".into(),
            microphone: None,
            mic_only: true,
            isolate_system_audio: true,
            hardware: "auto".into(),
            auto_submit: true,
            hands_free: false,
            push_to_talk: true,
            vad_threshold: 0.5,
            silence_ms: 800,
            extra_model_dirs: Vec::new(),
            call_view: true,
            auto_stop_silence_secs: 8,
            hands_free_timeout_secs: 300,
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
pub struct SearchSettings {
    /// Built-in DuckDuckGo web search offered to the model as a tool.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Results per search (1-10).
    pub max_results: u32,
    /// Optional SearXNG instance ("https://searx.example.org"). When set it is
    /// asked first: it answers JSON and never shows a captcha.
    pub searxng_url: String,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self { enabled: true, max_results: 5, searxng_url: String::new() }
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
    pub search: SearchSettings,
    pub last_conversation_id: Option<String>,
    /// Schema version of the stored settings, used for one-time migrations.
    pub version: u32,
}

impl Settings {
    /// Clamp values into sane ranges; applied on every save.
    pub fn sanitize(&mut self) {
        fn is_modifier_token(t: &str) -> bool {
            matches!(
                t.trim().to_ascii_uppercase().as_str(),
                "ALT" | "OPTION"
                    | "CONTROL" | "CTRL" | "COMMANDORCONTROL" | "COMMANDORCTRL" | "CMDORCTRL" | "CMDORCONTROL"
                    | "SHIFT"
                    | "SUPER" | "META" | "COMMAND" | "CMD" | "WIN"
            )
        }
        fn is_single_modifier(keys: &str) -> bool {
            let tokens: Vec<&str> = keys.split('+').map(str::trim).filter(|t| !t.is_empty()).collect();
            tokens.len() == 1 && tokens.iter().all(|t| is_modifier_token(t))
        }
        // Single Alt/Ctrl/Shift/Super alone fires on every normal press
        // (Alt+Tab, etc). Migrate legacy values to safe defaults.
        if is_single_modifier(&self.general.global_shortcut) {
            self.general.global_shortcut = "CommandOrControl+Space".into();
        }
        if is_single_modifier(&self.general.push_to_talk_shortcut) {
            self.general.push_to_talk_shortcut = "CommandOrControl+Shift+Space".into();
        }
        if is_single_modifier(&self.dictation.shortcut) {
            self.dictation.shortcut = "CommandOrControl+Alt+Space".into();
        }
        if self.general.global_shortcut.trim().is_empty() {
            self.general.global_shortcut = "CommandOrControl+Space".into();
        }
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
        if !matches!(self.general.accent.as_str(), "teal" | "blue" | "green" | "amber" | "rose" | "slate") {
            self.general.accent = "teal".into();
        }
        self.general.assistant_name = self.general.assistant_name.trim().chars().take(40).collect();
        if self.general.assistant_name.is_empty() {
            self.general.assistant_name = "Local Assistant".into();
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
        self.ai.api_key = self.ai.api_key.take().map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
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
        if self.stt.auto_stop_silence_secs != 0 {
            self.stt.auto_stop_silence_secs = self.stt.auto_stop_silence_secs.clamp(2, 120);
        }
        if self.stt.hands_free_timeout_secs != 0 {
            self.stt.hands_free_timeout_secs = self.stt.hands_free_timeout_secs.clamp(15, 3600);
        }
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
            if s.version < 3 {
                // v3: system-audio isolation is on by default, so the microphone
                // no longer transcribes what the speakers are playing.
                s.stt.isolate_system_audio = true;
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

    #[test]
    fn migration_turns_on_system_audio_isolation() {
        let db = Arc::new(Db::open_in_memory().unwrap());
        db.set_kv(KEY, r#"{"version":2,"stt":{"isolateSystemAudio":false}}"#).unwrap();
        let store = SettingsStore::load(db.clone()).unwrap();
        assert!(store.get().stt.isolate_system_audio);
        // A later explicit opt-out is respected.
        store.update(|s| s.stt.isolate_system_audio = false).unwrap();
        assert!(!SettingsStore::load(db).unwrap().get().stt.isolate_system_audio);
    }

    #[test]
    fn mic_only_defaults_true_for_old_settings() {
        let s = Settings::default();
        assert!(s.stt.mic_only);
        // Stored settings from before the flag existed must also get true.
        let old: Settings = serde_json::from_str(r#"{"stt":{"microphone":null}}"#).unwrap();
        assert!(old.stt.mic_only);
    }
}
