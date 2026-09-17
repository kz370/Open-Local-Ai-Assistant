//! Local audio I/O: device discovery, microphone capture (16 kHz mono) and playback.

pub mod capture;
pub mod devices;
pub mod playback;

pub const STT_SAMPLE_RATE: u32 = 16_000;

/// Root-mean-square level mapped to a 0..1 meter value (roughly -60 dB..0 dB).
pub fn level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    if rms <= 1e-6 {
        return 0.0;
    }
    let db = 20.0 * rms.log10();
    ((db + 60.0) / 60.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::level;

    #[test]
    fn level_scale() {
        assert_eq!(level(&[]), 0.0);
        assert_eq!(level(&[0.0; 100]), 0.0);
        assert!(level(&[1.0; 100]) > 0.99);
        let quiet = level(&[0.01; 100]);
        assert!(quiet > 0.2 && quiet < 0.5);
    }
}
