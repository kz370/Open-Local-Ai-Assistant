//! Resolves which LM Studio model to use for a request (automatic or manual),
//! caching discovery results briefly and loading the model when needed.

use crate::errors::{AppError, AppResult};
use crate::services::ai::model_selector::{select_model, ModelSelection};
use crate::services::ai::{AiService, ModelInfo};
use crate::services::hardware::{self, HardwareInfo};
use crate::settings::AiSettings;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OnceCell};

pub struct ModelResolver {
    ai: Arc<dyn AiService>,
    hardware: OnceCell<HardwareInfo>,
    cache: Mutex<Option<(Instant, Vec<ModelInfo>)>>,
}

#[derive(Debug, Clone)]
pub struct ResolvedModel {
    pub id: String,
    pub info: Option<ModelInfo>,
    pub selection: Option<ModelSelection>,
}

const CACHE_TTL: Duration = Duration::from_secs(20);

impl ModelResolver {
    pub fn new(ai: Arc<dyn AiService>) -> Self {
        Self { ai, hardware: OnceCell::new(), cache: Mutex::new(None) }
    }

    pub fn with_hardware(ai: Arc<dyn AiService>, hw: HardwareInfo) -> Self {
        let r = Self::new(ai);
        let _ = r.hardware.set(hw);
        r
    }

    pub async fn hardware(&self) -> HardwareInfo {
        self.hardware
            .get_or_init(|| async { tokio::task::spawn_blocking(hardware::detect).await.unwrap_or_default() })
            .await
            .clone()
    }

    pub async fn invalidate(&self) {
        *self.cache.lock().await = None;
    }

    pub async fn models(&self, force: bool) -> AppResult<Vec<ModelInfo>> {
        let mut cache = self.cache.lock().await;
        if !force {
            if let Some((at, models)) = cache.as_ref() {
                if at.elapsed() < CACHE_TTL {
                    return Ok(models.clone());
                }
            }
        }
        let models = self.ai.list_models().await?;
        *cache = Some((Instant::now(), models.clone()));
        Ok(models)
    }

    pub async fn auto_selection(&self) -> AppResult<Option<ModelSelection>> {
        let models = self.models(false).await?;
        Ok(select_model(&models, &self.hardware().await))
    }

    /// Resolve the chat model, loading it in LM Studio if it is not loaded yet.
    pub async fn resolve(&self, ai: &AiSettings) -> AppResult<ResolvedModel> {
        let models = self.models(false).await?;
        if ai.model_mode == "manual" {
            if let Some(id) = ai.model.as_deref().filter(|s| !s.is_empty()) {
                let info = models.iter().find(|m| m.id == id).cloned();
                if info.is_none() && !models.is_empty() {
                    return Err(AppError::LmStudio(format!("selected model '{id}' is not available in LM Studio")));
                }
                self.ensure_loaded(info.as_ref(), ai.context_length).await;
                return Ok(ResolvedModel { id: id.to_string(), info, selection: None });
            }
        }
        let selection = select_model(&models, &self.hardware().await).ok_or(AppError::NoModel)?;
        let info = models.iter().find(|m| m.id == selection.model_id).cloned();
        self.ensure_loaded(info.as_ref(), ai.context_length).await;
        Ok(ResolvedModel { id: selection.model_id.clone(), info, selection: Some(selection) })
    }

    async fn ensure_loaded(&self, info: Option<&ModelInfo>, ctx: Option<u32>) {
        let Some(info) = info else { return };
        if info.loaded {
            return;
        }
        // Failure is not fatal: LM Studio's JIT loading may still serve the request.
        match self.ai.load_model(&info.id, ctx).await {
            Ok(()) => {
                tracing::info!(model = %info.id, "model loaded");
                self.invalidate().await;
            }
            Err(e) => tracing::warn!(model = %info.id, error = %e, "explicit model load failed; relying on JIT load"),
        }
    }
}
