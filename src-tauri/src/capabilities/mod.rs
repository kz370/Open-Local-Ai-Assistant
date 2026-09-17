//! LocalCapabilityManager: one scan of everything the assistant can use locally.

use crate::services::ai::model_selector::ModelSelection;
use crate::services::ai::AiService;
use crate::services::audio::devices::{self, AudioDevice};
use crate::services::hardware::HardwareInfo;
use crate::services::language::Lang;
use crate::services::mcp::McpManager;
use crate::services::models::catalog::ModelKind;
use crate::services::models::{recommend, InstalledModel, ModelStore, Recommendation};
use crate::services::stt::SttService;
use crate::services::tts::TtsService;
use crate::settings::Settings;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LmStudioCapability {
    pub connected: bool,
    pub server_url: String,
    pub api: Option<String>,
    pub model_count: usize,
    pub error_code: Option<String>,
    pub error_detail: Option<String>,
    pub selection: Option<ModelSelection>,
    pub loaded_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceCapability {
    pub stt_model: Option<String>,
    pub vad_ready: bool,
    pub tts_en: Option<String>,
    pub tts_ar: Option<String>,
    pub tts_de: Option<String>,
    pub installed: Vec<InstalledModel>,
    pub recommendation: Recommendation,
    /// This build runs voice models on the CPU.
    pub acceleration: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCapability {
    pub enabled: usize,
    pub connected: usize,
    pub internet: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityReport {
    pub lm_studio: LmStudioCapability,
    pub hardware: HardwareInfo,
    pub voice: VoiceCapability,
    pub microphones: Vec<AudioDevice>,
    pub audio_outputs: Vec<AudioDevice>,
    pub mcp: McpCapability,
}

pub struct LocalCapabilityManager {
    pub ai: Arc<dyn AiService>,
    pub resolver: Arc<crate::services::chat::ModelResolver>,
    pub store: Arc<ModelStore>,
    pub stt: Arc<SttService>,
    pub tts: Arc<TtsService>,
    pub mcp: Arc<McpManager>,
    pub hardware: HardwareInfo,
}

impl LocalCapabilityManager {
    pub async fn detect_lm_studio(&self) -> LmStudioCapability {
        match self.ai.test_connection().await {
            Ok(status) => {
                let models = self.resolver.models(true).await.unwrap_or_default();
                let selection = crate::services::ai::model_selector::select_model(&models, &self.hardware);
                LmStudioCapability {
                    connected: true,
                    server_url: status.server_url,
                    api: Some(status.api),
                    model_count: status.model_count,
                    error_code: None,
                    error_detail: None,
                    selection,
                    loaded_models: models.iter().filter(|m| m.loaded).map(|m| m.id.clone()).collect(),
                }
            }
            Err(e) => LmStudioCapability {
                connected: false,
                server_url: String::new(),
                api: None,
                model_count: 0,
                error_code: Some(e.code().into()),
                error_detail: Some(e.to_string()),
                selection: None,
                loaded_models: vec![],
            },
        }
    }

    pub fn detect_voice(&self, settings: &Settings) -> VoiceCapability {
        let installed = self.store.installed();
        let voices = self.tts.voices();
        let gender = settings.tts.preferred_gender.clone();
        let voice_name = |lang: Lang, pref: &str| crate::services::tts::voices::select_voice(&voices, lang, pref, &gender).map(|v| v.name);
        VoiceCapability {
            stt_model: self.stt.resolve_model(&settings.stt).map(|m| m.id),
            vad_ready: installed.iter().any(|m| m.kind == ModelKind::Vad),
            tts_en: voice_name(Lang::En, &settings.tts.voice_en),
            tts_ar: voice_name(Lang::Ar, &settings.tts.voice_ar),
            tts_de: voice_name(Lang::De, &settings.tts.voice_de),
            installed,
            recommendation: recommend(&self.hardware),
            acceleration: "cpu".into(),
        }
    }

    pub async fn scan(&self, settings: &Settings) -> CapabilityReport {
        let lm_studio = self.detect_lm_studio().await;
        let (microphones, audio_outputs) = tokio::task::spawn_blocking(|| (devices::list_inputs(), devices::list_outputs()))
            .await
            .unwrap_or_default();
        let statuses = self.mcp.statuses().await.unwrap_or_default();
        CapabilityReport {
            lm_studio,
            hardware: self.hardware.clone(),
            voice: self.detect_voice(settings),
            microphones,
            audio_outputs,
            mcp: McpCapability {
                enabled: statuses.iter().filter(|s| s.config.enabled).count(),
                connected: statuses.iter().filter(|s| s.state == "connected").count(),
                internet: self.mcp.internet_available().await,
            },
        }
    }
}
