//! Automatic LM Studio model selection.
//!
//! Picks a *practical* model for this machine — not the largest one. Already
//! loaded models are strongly preferred (instant responses, no extra memory),
//! then models that fit comfortably in VRAM (or RAM on CPU-only machines),
//! with bonuses for tool-use training (needed for MCP), a useful context size,
//! a mid-range parameter count and a sensible quantization.

use super::ModelInfo;
use crate::services::hardware::HardwareInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelSelection {
    pub model_id: String,
    pub needs_load: bool,
    pub score: i32,
    /// Machine-readable reasons, rendered by the UI.
    pub reasons: Vec<String>,
}

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Parses LM Studio `params_string` values such as "9B", "1.5B", "30B-A3B", "270M".
pub fn parse_params_billions(s: &str) -> Option<f64> {
    let s = s.trim().to_ascii_uppercase();
    let head = s.split(['-', ' ']).next()?;
    let (num, unit) = head.split_at(head.find(|c: char| c.is_ascii_alphabetic())?);
    let n: f64 = num.parse().ok()?;
    match unit {
        "B" => Some(n),
        "M" => Some(n / 1000.0),
        _ => None,
    }
}

fn params_of(m: &ModelInfo) -> Option<f64> {
    if let Some(p) = m.params.as_deref().and_then(parse_params_billions) {
        return Some(p);
    }
    // Fall back to the id, e.g. "qwen/qwen3-14b" or "gemma-3-12b".
    m.id.to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
        .filter_map(|tok| tok.strip_suffix('b').and_then(|n| n.parse::<f64>().ok()))
        .filter(|n| *n > 0.1 && *n < 1000.0)
        .last()
}

pub fn score_model(m: &ModelInfo, hw: &HardwareInfo) -> (i32, Vec<String>) {
    let mut score = 0;
    let mut reasons = Vec::new();

    if m.loaded {
        score += 40;
        reasons.push("already_loaded".into());
    }

    let vram = hw.best_vram() as f64;
    let ram = hw.total_ram_bytes as f64;
    let has_gpu = vram >= 3.5 * GB;

    if let Some(size) = m.size_bytes.map(|s| s as f64) {
        let fits_gpu = has_gpu && size * 1.15 <= vram * 0.9;
        let fits_ram = size * 1.2 <= ram * 0.5;
        if fits_gpu {
            score += 30;
            reasons.push("fits_gpu".into());
        } else if fits_ram && !m.loaded {
            score += if has_gpu { 5 } else { 12 };
            reasons.push(if has_gpu { "partial_offload" } else { "fits_ram" }.into());
        } else if !m.loaded {
            score -= 100;
            reasons.push("too_large".into());
        }
    }

    match params_of(m) {
        Some(p) => {
            let bonus = if has_gpu {
                match p {
                    p if p < 2.0 => -25,
                    p if p < 4.0 => -10,
                    p if p < 7.0 => 10,
                    p if p <= 14.5 => 20,
                    p if p <= 21.0 => 8,
                    _ => 0,
                }
            } else {
                match p {
                    p if p < 2.0 => -15,
                    p if p < 3.0 => 0,
                    p if p <= 8.5 => 20,
                    p if p <= 14.5 => 5,
                    _ => -15,
                }
            };
            score += bonus;
            if bonus >= 20 {
                reasons.push("practical_size".into());
            }
        }
        None => reasons.push("unknown_size".into()),
    }

    if m.tool_use {
        score += 15;
        reasons.push("tool_use".into());
    }

    let ctx = m.loaded_context_length.or(m.max_context_length).unwrap_or(0);
    if ctx >= 16_000 {
        score += 5;
    } else if ctx > 0 && ctx < 8_000 {
        score -= 10;
        reasons.push("small_context".into());
    }

    if let Some(bits) = m.bits_per_weight {
        if bits <= 2.0 {
            score -= 8;
            reasons.push("low_precision".into());
        } else if (3.5..=6.5).contains(&bits) {
            score += 5;
        }
    }

    let lid = m.id.to_ascii_lowercase();
    if lid.contains("coder") || lid.contains("commit") {
        score -= 6;
        reasons.push("specialized".into());
    }

    (score, reasons)
}

pub fn select_model(models: &[ModelInfo], hw: &HardwareInfo) -> Option<ModelSelection> {
    models
        .iter()
        .filter(|m| m.is_chat_model())
        .map(|m| {
            let (score, reasons) = score_model(m, hw);
            (m, score, reasons)
        })
        // Highest score wins; ties broken by id for determinism.
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.id.cmp(&a.0.id)))
        .map(|(m, score, reasons)| ModelSelection {
            model_id: m.id.clone(),
            needs_load: !m.loaded,
            score,
            reasons,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::hardware::GpuInfo;

    fn gpu_machine(vram_gb: u64) -> HardwareInfo {
        HardwareInfo {
            total_ram_bytes: 96 * GB as u64,
            physical_cores: 6,
            gpus: vec![GpuInfo { name: "RTX".into(), vendor: "nvidia".into(), vram_bytes: vram_gb * GB as u64 }],
            ..Default::default()
        }
    }

    fn cpu_machine(ram_gb: u64) -> HardwareInfo {
        HardwareInfo { total_ram_bytes: ram_gb * GB as u64, physical_cores: 4, ..Default::default() }
    }

    fn model(id: &str, params: &str, size_gb: f64, tool: bool, loaded: bool) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            display_name: id.into(),
            kind: "llm".into(),
            size_bytes: Some((size_gb * GB) as u64),
            params: Some(params.into()),
            bits_per_weight: Some(4.0),
            max_context_length: Some(32768),
            loaded,
            tool_use: tool,
            ..Default::default()
        }
    }

    #[test]
    fn parses_param_strings() {
        assert_eq!(parse_params_billions("9B"), Some(9.0));
        assert_eq!(parse_params_billions("1.5B"), Some(1.5));
        assert_eq!(parse_params_billions("30B-A3B"), Some(30.0));
        assert_eq!(parse_params_billions("270M"), Some(0.27));
        assert_eq!(parse_params_billions("abc"), None);
    }

    #[test]
    fn prefers_loaded_practical_model() {
        let models = vec![
            model("big-27b", "27B", 16.0, true, false),
            model("qwen-9b", "9B", 6.5, true, true),
            model("qwen-14b", "14B", 9.0, true, false),
        ];
        let s = select_model(&models, &gpu_machine(12)).unwrap();
        assert_eq!(s.model_id, "qwen-9b");
        assert!(!s.needs_load);
    }

    #[test]
    fn does_not_pick_largest_when_nothing_loaded() {
        let models = vec![
            model("huge-70b", "70B", 40.0, true, false),
            model("mid-12b", "12B", 7.5, true, false),
            model("tiny-1b", "1B", 0.8, true, false),
        ];
        let s = select_model(&models, &gpu_machine(12)).unwrap();
        assert_eq!(s.model_id, "mid-12b");
        assert!(s.needs_load);
    }

    #[test]
    fn cpu_only_prefers_small_models() {
        let models = vec![model("mid-14b", "14B", 9.0, true, false), model("small-7b", "7B", 4.5, true, false)];
        let s = select_model(&models, &cpu_machine(16)).unwrap();
        assert_eq!(s.model_id, "small-7b");
    }

    #[test]
    fn excludes_embeddings_and_handles_empty() {
        let mut e = model("nomic-embed", "137M", 0.1, false, true);
        e.kind = "embedding".into();
        assert!(select_model(&[e.clone()], &gpu_machine(12)).is_none());
        assert!(select_model(&[], &gpu_machine(12)).is_none());
    }

    #[test]
    fn tool_use_breaks_near_ties() {
        let models = vec![model("a-8b", "8B", 5.0, false, false), model("b-8b", "8B", 5.0, true, false)];
        assert_eq!(select_model(&models, &gpu_machine(12)).unwrap().model_id, "b-8b");
    }

    #[test]
    fn unknown_metadata_still_selects_something() {
        let models = vec![ModelInfo { id: "some-model".into(), kind: "unknown".into(), ..Default::default() }];
        assert_eq!(select_model(&models, &cpu_machine(8)).unwrap().model_id, "some-model");
    }

    #[test]
    fn params_from_id_fallback() {
        let m = ModelInfo { id: "google/gemma-3-12b".into(), ..Default::default() };
        assert_eq!(params_of(&m), Some(12.0));
    }
}
