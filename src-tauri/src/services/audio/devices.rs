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

/// Names that capture system output instead of a microphone
/// (Stereo Mix, loopback, monitor sources). Case-insensitive match.
fn is_loopback_name(name: &str) -> bool {
    const MARKS: &[&str] = &[
        "stereo mix",
        "loopback",
        "what you hear",
        "what u hear",
        "wave out",
        "waveout",
        "monitor of",
        ".monitor",
        "desktop audio",
        "system audio",
    ];
    let lower = name.to_lowercase();
    MARKS.iter().any(|m| lower.contains(m))
}

pub fn list_inputs(mic_only: bool) -> Vec<AudioDevice> {
    let host = cpal::default_host();
    let default_id = host.default_input_device().and_then(|d| d.id().ok()).map(|i| i.to_string());
    host.input_devices()
        .map(|it| {
            it.filter_map(|d| describe(&d, &default_id))
                .filter(|d| !mic_only || !is_loopback_name(&d.name))
                .collect()
        })
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
/// With `mic_only`, loopback devices (Stereo Mix etc.) are skipped in favor of
/// the first real microphone.
pub fn input_device(id: Option<&str>, mic_only: bool) -> AppResult<cpal::Device> {
    let host = cpal::default_host();
    let usable = |d: &cpal::Device| -> bool {
        if !mic_only {
            return true;
        }
        let name = d.description().map(|x| x.name().to_string()).unwrap_or_else(|_| d.to_string());
        !is_loopback_name(&name)
    };
    if let Some(id) = id {
        if let Some(d) = host
            .input_devices()
            .ok()
            .and_then(|mut it| it.find(|d| d.id().map(|i| i.to_string() == id).unwrap_or(false)))
        {
            if usable(&d) {
                return Ok(d);
            }
            tracing::warn!("configured microphone is a loopback device, using a microphone instead");
        } else {
            tracing::warn!("configured microphone not found, using default");
        }
    }
    if mic_only {
        if let Some(d) = host.input_devices().ok().and_then(|mut it| it.find(|d| usable(d))) {
            return Ok(d);
        }
        return Err(AppError::Audio("no microphone found (loopback devices excluded)".into()));
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

#[cfg(test)]
mod tests {
    use super::is_loopback_name;

    #[test]
    fn loopback_names_detected() {
        for name in [
            "Stereo Mix (Realtek High Definition Audio)",
            "CABLE Output (VB-Audio Virtual Cable)",
            "Monitor of Built-in Audio",
            "What You Hear",
            "Desktop Audio",
            "System Audio",
            "Wave Out Mix",
        ] {
            // Virtual-cable style names are NOT in the blocklist (real device).
            if name.contains("CABLE") {
                assert!(!is_loopback_name(name), "{name}");
            } else {
                assert!(is_loopback_name(name), "{name}");
            }
        }
        for name in ["Microphone (Realtek High Definition Audio)", "Headset Microphone", "USB Audio Device", "Yeti Stereo Microphone"] {
            assert!(!is_loopback_name(name), "{name}");
        }
    }
}
