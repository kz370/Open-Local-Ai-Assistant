use crate::errors::{AppError, AppResult};
use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

fn describe(d: &cpal::Device, default_id: &Option<String>) -> Option<AudioDevice> {
    let id = d.id().ok()?.to_string();
    let name = d.description().map(|x| x.name().to_string()).unwrap_or_else(|_| d.to_string());
    Some(AudioDevice { is_default: default_id.as_deref() == Some(id.as_str()), id, name })
}

pub fn list_inputs() -> Vec<AudioDevice> {
    let host = cpal::default_host();
    let default_id = host.default_input_device().and_then(|d| d.id().ok()).map(|i| i.to_string());
    host.input_devices()
        .map(|it| it.filter_map(|d| describe(&d, &default_id)).collect())
        .unwrap_or_default()
}

pub fn list_outputs() -> Vec<AudioDevice> {
    let host = cpal::default_host();
    let default_id = host.default_output_device().and_then(|d| d.id().ok()).map(|i| i.to_string());
    host.output_devices()
        .map(|it| it.filter_map(|d| describe(&d, &default_id)).collect())
        .unwrap_or_default()
}

/// Finds a device by id; `None`, or an id that disappeared, selects the system default.
pub fn input_device(id: Option<&str>) -> AppResult<cpal::Device> {
    let host = cpal::default_host();
    if let Some(id) = id {
        if let Some(d) = host
            .input_devices()
            .ok()
            .and_then(|mut it| it.find(|d| d.id().map(|i| i.to_string() == id).unwrap_or(false)))
        {
            return Ok(d);
        }
        tracing::warn!("configured microphone not found, using default");
    }
    host.default_input_device().ok_or_else(|| AppError::Audio("no microphone found".into()))
}

pub fn output_device(id: Option<&str>) -> AppResult<cpal::Device> {
    let host = cpal::default_host();
    if let Some(id) = id {
        if let Some(d) = host
            .output_devices()
            .ok()
            .and_then(|mut it| it.find(|d| d.id().map(|i| i.to_string() == id).unwrap_or(false)))
        {
            return Ok(d);
        }
        tracing::warn!("configured audio output not found, using default");
    }
    host.default_output_device().ok_or_else(|| AppError::Audio("no audio output device found".into()))
}
