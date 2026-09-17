//! Application state shared by commands, tray and shortcut handlers.

use crate::capabilities::LocalCapabilityManager;
use crate::database::Db;
use crate::services::ai::lmstudio::LmStudioService;
use crate::services::chat::{ChatEngine, ModelResolver};
use crate::services::hardware::HardwareInfo;
use crate::services::mcp::McpManager;
use crate::services::models::ModelStore;
use crate::services::stt::session::VoiceSessions;
use crate::services::stt::SttService;
use crate::services::tts::TtsService;
use crate::settings::SettingsStore;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub models_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub database: PathBuf,
}

pub struct AppState {
    pub paths: AppPaths,
    pub db: Arc<Db>,
    pub settings: Arc<SettingsStore>,
    pub lmstudio: Arc<LmStudioService>,
    pub resolver: Arc<ModelResolver>,
    pub chat: Arc<ChatEngine>,
    pub mcp: Arc<McpManager>,
    pub models: Arc<ModelStore>,
    pub stt: Arc<SttService>,
    pub tts: Arc<TtsService>,
    pub voice: Arc<VoiceSessions>,
    pub hardware: HardwareInfo,
    pub downloads: Mutex<HashMap<String, CancellationToken>>,
    pub dictation_busy: AtomicBool,
    /// Set by Esc/X cancel; consumed by run_dictation before any insert.
    pub dictation_cancel: AtomicBool,
    pub shortcut_errors: Mutex<Vec<String>>,
}

impl AppState {
    pub fn capabilities(&self) -> LocalCapabilityManager {
        LocalCapabilityManager {
            ai: self.lmstudio.clone(),
            resolver: self.resolver.clone(),
            store: self.models.clone(),
            stt: self.stt.clone(),
            tts: self.tts.clone(),
            mcp: self.mcp.clone(),
            hardware: self.hardware.clone(),
        }
    }
}
