//! Persistent application settings (stored as one JSON document in SQLite).
//!
//! Every field has a serde default so settings written by older versions keep
//! loading after new fields are added.

pub mod backup;

use crate::database::Db;
use crate::errors::AppResult;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

const KEY: &str = "app_settings";
pub const DEFAULT_LMSTUDIO_URL: &str = "http://localhost:1234/v1";

/// Supported chat providers: (id, display name, default OpenAI-compatible base URL).
/// Everything except `lmstudio` is a hosted API that needs an API key.
pub const PROVIDERS: &[(&str, &str, &str)] = &[
    ("lmstudio", "LM Studio", DEFAULT_LMSTUDIO_URL),
    ("openrouter", "OpenRouter", "https://openrouter.ai/api/v1"),
    ("groq", "Groq", "https://api.groq.com/openai/v1"),
    ("gemini", "Google Gemini API", "https://generativelanguage.googleapis.com/v1beta/openai"),
    ("huggingface", "Hugging Face", "https://router.huggingface.co/v1"),
    ("cerebras", "Cerebras", "https://api.cerebras.ai/v1"),
];

pub fn provider_name(id: &str) -> &'static str {
    PROVIDERS.iter().find(|p| p.0 == id).map_or("LM Studio", |p| p.1)
}

pub fn provider_default_url(id: &str) -> &'static str {
    PROVIDERS.iter().find(|p| p.0 == id).map_or(DEFAULT_LMSTUDIO_URL, |p| p.2)
}
const CURRENT_VERSION: u32 = 4;

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
    /// Load the speech, voice and chat models in the background at startup,
    /// so the first use does not wait for them (costs RAM/VRAM while idle).
    pub preload_models: bool,
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
            preload_models: true,
            always_on_top: false,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderProfile {
    pub server_url: String,
    pub api_key: Option<String>,
    pub model: Option<String>,
    /// "auto" | "manual"; empty means "auto".
    pub model_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AiSettings {
    /// One of the ids in [`PROVIDERS`]: "lmstudio" | "openrouter" | "groq" |
    /// "gemini" | "huggingface" | "cerebras".
    pub provider: String,
    pub server_url: String,
    /// Bearer token sent as `Authorization: Bearer <key>`. LM Studio itself
    /// ignores it; the hosted providers all require one.
    pub api_key: Option<String>,
    /// Remembered URL / key / model of the providers that are not currently
    /// selected, so switching back restores them. Keyed by provider id.
    pub provider_profiles: std::collections::BTreeMap<String, ProviderProfile>,
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
    /// Model ids left out of the chat window's model picker.
    pub hidden_models: Vec<String>,
    /// Pasted text longer than this many characters is attached as a text file
    /// instead of filling the composer. 0 turns the behaviour off.
    #[serde(default = "default_paste_as_file_chars")]
    pub paste_as_file_chars: u32,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: "lmstudio".into(),
            server_url: DEFAULT_LMSTUDIO_URL.into(),
            api_key: None,
            provider_profiles: Default::default(),
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
            hidden_models: Vec::new(),
            paste_as_file_chars: default_paste_as_file_chars(),
        }
    }
}

/// One entry in the user-editable language list. `en`/`ar`/`de` ship as
/// built-ins; a user can add further codes, but only en/ar/de get real
/// detection/response support today (see `services::language::detect::Lang`,
/// still a closed 3-variant enum) — anything else only gets a TTS voice
/// preference slot in Settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LanguageEntry {
    pub code: String,
    pub display_name: String,
    /// "ltr" | "rtl"
    pub direction: String,
    /// Language hint passed to the STT recognizer; usually equal to `code`.
    pub stt_language: String,
    /// "auto" or a catalog voice id.
    pub tts_voice: String,
    /// True for the shipped en/ar/de entries; UI-only signal (nothing
    /// server-side depends on it besides never letting the list hit zero).
    pub built_in: bool,
}

impl LanguageEntry {
    fn builtins() -> Vec<LanguageEntry> {
        vec![
            LanguageEntry { code: "en".into(), display_name: "English".into(), direction: "ltr".into(), stt_language: "en".into(), tts_voice: "auto".into(), built_in: true },
            LanguageEntry { code: "ar".into(), display_name: "Arabic".into(), direction: "rtl".into(), stt_language: "ar".into(), tts_voice: "auto".into(), built_in: true },
            LanguageEntry { code: "de".into(), display_name: "German".into(), direction: "ltr".into(), stt_language: "de".into(), tts_voice: "auto".into(), built_in: true },
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LanguageSettings {
    /// "auto" or one of entries[].code
    pub response_language: String,
    pub entries: Vec<LanguageEntry>,
    /// When true, Arabic replies are asked to include full tashkeel
    /// (diacritics) instead of today's "write without diacritics" default.
    pub arabic_tashkeel_enabled: bool,
    /// Custom instruction used instead of the built-in default text when
    /// tashkeel is enabled. Empty = use the default instruction.
    pub arabic_tashkeel_instruction: String,
}

impl Default for LanguageSettings {
    fn default() -> Self {
        Self {
            response_language: "auto".into(),
            entries: LanguageEntry::builtins(),
            arabic_tashkeel_enabled: false,
            arabic_tashkeel_instruction: String::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

/// Roughly a page and a half of text: long enough that pasting a snippet still
/// lands in the composer, short enough that a pasted document becomes a file.
fn default_paste_as_file_chars() -> u32 {
    2000
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
            auto_submit: false,
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
    /// Superseded by `LanguageSettings.entries[].tts_voice`. Kept only so a
    /// pre-v4 settings blob still deserializes into *something*, which the v4
    /// migration then copies into the matching language entry; never read at
    /// runtime otherwise. Do not delete without a migration.
    #[serde(rename = "voiceEn")]
    pub legacy_voice_en: String,
    #[serde(rename = "voiceAr")]
    pub legacy_voice_ar: String,
    #[serde(rename = "voiceDe")]
    pub legacy_voice_de: String,
    pub speed: f32,
    pub volume: f32,
    /// None = automatic (system default output)
    pub output_device: Option<String>,
    /// "any" | "female" | "male" - used when a voice is chosen automatically.
    pub preferred_gender: String,
    /// Per-voice hardware override ("auto" | "cpu"), keyed by voice/model id.
    /// Absent key = "auto".
    pub voice_hardware: std::collections::BTreeMap<String, String>,
    /// Orpheus voices: language code -> LM Studio model that speaks it.
    /// A language without an entry uses the local voices.
    pub orpheus_models: std::collections::BTreeMap<String, String>,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            speak_responses: false,
            legacy_voice_en: "auto".into(),
            legacy_voice_ar: "auto".into(),
            legacy_voice_de: "auto".into(),
            speed: 1.0,
            volume: 1.0,
            output_device: None,
            preferred_gender: "any".into(),
            voice_hardware: Default::default(),
            orpheus_models: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchSettings {
    /// DuckDuckGo search. It and SearXNG are switched on independently; the
    /// web_search tool is offered while either one is on.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Results per search (1-10).
    pub max_results: u32,
    /// The user's own SearXNG instance ("http://localhost:8080"), used when
    /// `searxng_source` is "local".
    pub searxng_url: String,
    /// Switch for SearXNG; off keeps the choices but skips SearXNG entirely.
    pub searxng_enabled: bool,
    /// Where SearXNG comes from: "local" (the URL above) or "public" (an
    /// instance from the searx.space list).
    pub searxng_source: String,
    /// The public instance picked from searx.space; empty means "pick the
    /// fastest working ones automatically".
    pub searxng_public_url: String,
    /// Engine tried first when both are on: "searxng" or "duckduckgo"; the
    /// other one is the fallback.
    pub primary: String,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_results: 5,
            searxng_url: String::new(),
            searxng_enabled: true,
            searxng_source: "local".into(),
            searxng_public_url: String::new(),
            primary: "searxng".into(),
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
    /// Last dragged position of the dictation overlay (physical pixels), or
    /// None to auto-center it near the bottom of the screen.
    pub overlay_x: Option<i32>,
    pub overlay_y: Option<i32>,
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
            overlay_x: None,
            overlay_y: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SilmaSettings {
    /// "auto" | "cpu" — forces the Arabic SILMA voice off the GPU.
    pub hardware: String,
}

impl Default for SilmaSettings {
    fn default() -> Self {
        Self { hardware: "auto".into() }
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
    pub silma: SilmaSettings,
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
        // Sanitize the language entries themselves before anything below
        // validates references into them.
        let mut seen = std::collections::HashSet::new();
        self.language.entries = std::mem::take(&mut self.language.entries)
            .into_iter()
            .filter_map(|mut e| {
                e.code = e.code.trim().to_ascii_lowercase();
                if e.code.is_empty() || !seen.insert(e.code.clone()) {
                    return None;
                }
                e.display_name = e.display_name.trim().chars().take(40).collect();
                if e.display_name.is_empty() {
                    e.display_name = e.code.clone();
                }
                if !matches!(e.direction.as_str(), "ltr" | "rtl") {
                    e.direction = "ltr".into();
                }
                e.stt_language = e.stt_language.trim().to_ascii_lowercase();
                if e.stt_language.is_empty() {
                    e.stt_language = e.code.clone();
                }
                e.tts_voice = e.tts_voice.trim().to_string();
                if e.tts_voice.is_empty() {
                    e.tts_voice = "auto".into();
                }
                Some(e)
            })
            .take(12)
            .collect();
        if self.language.entries.is_empty() {
            self.language.entries = LanguageEntry::builtins();
        }
        let lang_ok = |s: &str, entries: &[LanguageEntry]| s == "auto" || entries.iter().any(|e| e.code == s);
        if !lang_ok(&self.language.response_language, &self.language.entries) {
            self.language.response_language = "auto".into();
        }
        if !lang_ok(&self.stt.language, &self.language.entries) {
            self.stt.language = "auto".into();
        }
        self.language.arabic_tashkeel_instruction = self.language.arabic_tashkeel_instruction.trim().chars().take(500).collect();
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
        if !PROVIDERS.iter().any(|p| p.0 == self.ai.provider) {
            self.ai.provider = "lmstudio".into();
        }
        self.ai.temperature = self.ai.temperature.clamp(0.0, 2.0);
        self.ai.server_url = self.ai.server_url.trim().trim_end_matches('/').to_string();
        if self.ai.server_url.is_empty() {
            self.ai.server_url = provider_default_url(&self.ai.provider).into();
        }
        self.ai.provider_profiles.retain(|id, _| PROVIDERS.iter().any(|p| p.0 == id));
        // Hosted providers list hundreds of models, so there is nothing sensible
        // to auto-pick from: the user always chooses one.
        if self.ai.provider != "lmstudio" {
            self.ai.model_mode = "manual".into();
        }
        self.ai.api_key = self.ai.api_key.take().map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
        if !matches!(self.ai.model_mode.as_str(), "auto" | "manual") {
            self.ai.model_mode = "auto".into();
        }
        self.ai.request_timeout_secs = self.ai.request_timeout_secs.clamp(10, 3600);
        if self.ai.paste_as_file_chars != 0 {
            self.ai.paste_as_file_chars = self.ai.paste_as_file_chars.clamp(200, 200_000);
        }
        self.ai.model_aliases = std::mem::take(&mut self.ai.model_aliases)
            .into_iter()
            .map(|(k, v)| (k, v.trim().chars().take(40).collect::<String>()))
            .filter(|(k, v)| !k.is_empty() && !v.is_empty())
            .collect();
        self.ai.hidden_models.retain(|m| !m.trim().is_empty());
        self.ai.hidden_models.sort();
        self.ai.hidden_models.dedup();
        self.tts.speed = self.tts.speed.clamp(0.5, 2.0);
        self.tts.volume = self.tts.volume.clamp(0.0, 1.0);
        if !matches!(self.stt.hardware.as_str(), "auto" | "cpu") {
            self.stt.hardware = "auto".into();
        }
        self.tts.voice_hardware = std::mem::take(&mut self.tts.voice_hardware)
            .into_iter()
            .filter(|(k, v)| !k.is_empty() && matches!(v.as_str(), "auto" | "cpu"))
            .collect();
        if !matches!(self.silma.hardware.as_str(), "auto" | "cpu") {
            self.silma.hardware = "auto".into();
        }
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
        if !matches!(self.search.searxng_source.as_str(), "local" | "public") {
            self.search.searxng_source = "local".into();
        }
        if !matches!(self.search.primary.as_str(), "searxng" | "duckduckgo") {
            self.search.primary = "searxng".into();
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
            if s.version < 4 {
                // v4: the fixed en/ar/de voice fields became a user-editable
                // language list. `entries` already defaulted to the builtins
                // (serde's container-level `default` fills missing fields), so
                // just carry each legacy voice choice into its matching entry.
                for (code, legacy) in [
                    ("en", s.tts.legacy_voice_en.clone()),
                    ("ar", s.tts.legacy_voice_ar.clone()),
                    ("de", s.tts.legacy_voice_de.clone()),
                ] {
                    if legacy != "auto" {
                        if let Some(e) = s.language.entries.iter_mut().find(|e| e.code == code) {
                            e.tts_voice = legacy;
                        }
                    }
                }
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
    fn sanitize_keeps_known_providers_and_forces_manual_for_hosted() {
        let mut s = Settings::default();
        s.ai.provider = "groq".into();
        s.ai.server_url = String::new();
        s.sanitize();
        assert_eq!(s.ai.provider, "groq");
        assert_eq!(s.ai.server_url, "https://api.groq.com/openai/v1");
        assert_eq!(s.ai.model_mode, "manual");
        s.ai.provider = "nope".into();
        s.sanitize();
        assert_eq!(s.ai.provider, "lmstudio");
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
    fn migration_carries_legacy_voices_into_language_entries() {
        let db = Arc::new(Db::open_in_memory().unwrap());
        db.set_kv(KEY, r#"{"version":3,"tts":{"voiceEn":"kokoro-en-v0_19:1","voiceAr":"piper-ar_JO-kareem-medium:0"}}"#).unwrap();
        let store = SettingsStore::load(db).unwrap();
        let s = store.get();
        assert_eq!(s.version, CURRENT_VERSION);
        let en = s.language.entries.iter().find(|e| e.code == "en").unwrap();
        assert_eq!(en.tts_voice, "kokoro-en-v0_19:1");
        let ar = s.language.entries.iter().find(|e| e.code == "ar").unwrap();
        assert_eq!(ar.tts_voice, "piper-ar_JO-kareem-medium:0");
        let de = s.language.entries.iter().find(|e| e.code == "de").unwrap();
        assert_eq!(de.tts_voice, "auto");
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
