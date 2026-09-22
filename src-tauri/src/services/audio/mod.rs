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

/// One-pole high-pass filter that attenuates steady low-frequency noise
/// (fan hum, AC rumble) before it reaches the level meter, VAD or
/// recognizer, so a running fan is less likely to be mistaken for speech.
pub struct HighPassFilter {
    alpha: f32,
    prev_in: f32,
    prev_out: f32,
}

impl HighPassFilter {
    /// `cutoff_hz` is the -3dB point below which content is attenuated.
    pub fn new(cutoff_hz: f32, sample_rate: u32) -> Self {
        let rc = 1.0 / (std::f32::consts::TAU * cutoff_hz);
        let dt = 1.0 / sample_rate as f32;
        Self { alpha: rc / (rc + dt), prev_in: 0.0, prev_out: 0.0 }
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            let out = self.alpha * (self.prev_out + *s - self.prev_in);
            self.prev_in = *s;
            self.prev_out = out;
            *s = out;
        }
    }
}

/// Number of bands in [`spectrum`].
pub const SPECTRUM_BANDS: usize = 24;

/// Analysis window length of [`spectrum`].
const N: usize = 512;

static HANN: std::sync::LazyLock<Vec<f32>> =
    std::sync::LazyLock::new(|| (0..N).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos()).collect());

/// Cosine and sine of every (bin, sample) pair, built once: the meter runs in
/// the microphone callback, where trigonometry per sample would be wasteful.
fn twiddles(top_bin: usize) -> &'static (Vec<f32>, Vec<f32>) {
    static TABLES: std::sync::OnceLock<(Vec<f32>, Vec<f32>)> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let (mut cos, mut sin) = (Vec::with_capacity((top_bin + 1) * N), Vec::with_capacity((top_bin + 1) * N));
        for k in 0..=top_bin {
            for i in 0..N {
                let a = std::f32::consts::TAU * k as f32 * i as f32 / N as f32;
                cos.push(a.cos());
                sin.push(a.sin());
            }
        }
        (cos, sin)
    })
}

/// Voice spectrum for the meter: `SPECTRUM_BANDS` log-spaced bands between
/// 80 Hz and 4 kHz (16 kHz input), each mapped to 0..1 like [`level`].
pub fn spectrum(samples: &[f32]) -> Vec<f32> {
    const LOW_HZ: f32 = 80.0;
    const HIGH_HZ: f32 = 4000.0;
    if samples.len() < N {
        return vec![0.0; SPECTRUM_BANDS];
    }
    // Hann-windowed DFT over the latest N samples, only up to HIGH_HZ.
    let frame = &samples[samples.len() - N..];
    let bin_hz = STT_SAMPLE_RATE as f32 / N as f32;
    let top_bin = (HIGH_HZ / bin_hz) as usize;
    let tables = twiddles(top_bin);
    let window: Vec<f32> = (0..N).map(|i| frame[i] * HANN[i]).collect();
    let power: Vec<f32> = (0..=top_bin)
        .map(|k| {
            let (cos, sin) = (&tables.0[k * N..(k + 1) * N], &tables.1[k * N..(k + 1) * N]);
            let (mut re, mut im) = (0.0f32, 0.0f32);
            for i in 0..N {
                re += window[i] * cos[i];
                im -= window[i] * sin[i];
            }
            // Amplitude normalised for the Hann window (sum = N/2).
            let mag = (re * re + im * im).sqrt() * 4.0 / N as f32;
            mag * mag
        })
        .collect();
    let ratio = (HIGH_HZ / LOW_HZ).powf(1.0 / SPECTRUM_BANDS as f32);
    (0..SPECTRUM_BANDS)
        .map(|b| {
            let lo_hz = LOW_HZ * ratio.powi(b as i32);
            let hi_hz = lo_hz * ratio;
            let lo = ((lo_hz / bin_hz) as usize).max(1);
            let hi = ((hi_hz / bin_hz) as usize).clamp(lo, top_bin);
            let avg = power[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
            if avg <= 1e-12 {
                return 0.0;
            }
            // Speech loses energy with pitch; lift the highs so every bar can move.
            let tilt = 3.0 * (((lo_hz + hi_hz) / 2.0) / 250.0).log2().max(0.0);
            let db = 10.0 * avg.log10() + tilt;
            ((db + 75.0) / 55.0).clamp(0.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{level, spectrum, HighPassFilter, SPECTRUM_BANDS, STT_SAMPLE_RATE};

    #[test]
    fn high_pass_cuts_hum_keeps_speech() {
        // Skip the filter's startup transient (a few cycles at the lowest
        // frequency involved) so RMS reflects steady-state response only.
        let tone = |hz: f32| -> Vec<f32> { (0..1600).map(|i| 0.3 * (std::f32::consts::TAU * hz * i as f32 / STT_SAMPLE_RATE as f32).sin()).collect() };
        let rms = |s: &[f32]| (s[400..].iter().map(|v| v * v).sum::<f32>() / (s.len() - 400) as f32).sqrt();

        let mut hum = tone(50.0);
        let hum_in = rms(&hum);
        HighPassFilter::new(100.0, STT_SAMPLE_RATE).process(&mut hum);
        assert!(rms(&hum) < hum_in * 0.5, "50 Hz hum should be heavily attenuated");

        let mut voice = tone(300.0);
        let voice_in = rms(&voice);
        HighPassFilter::new(100.0, STT_SAMPLE_RATE).process(&mut voice);
        assert!(rms(&voice) > voice_in * 0.9, "300 Hz speech content should pass through mostly intact");
    }

    /// `cargo test spectrum_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn spectrum_cost() {
        let audio: Vec<f32> = (0..800).map(|i| (i as f32 * 0.05).sin() * 0.2).collect();
        spectrum(&audio);
        let started = std::time::Instant::now();
        for _ in 0..200 {
            spectrum(&audio);
        }
        // 200 calls = 10 seconds of microphone input at 20 updates per second.
        println!("{:.2} ms per update", started.elapsed().as_secs_f32() * 1000.0 / 200.0);
    }

    #[test]
    fn spectrum_follows_pitch() {
        assert_eq!(spectrum(&[]), vec![0.0; SPECTRUM_BANDS]);
        assert!(spectrum(&[0.0; 800]).iter().all(|&v| v == 0.0));
        let tone = |hz: f32| (0..800).map(|i| 0.3 * (std::f32::consts::TAU * hz * i as f32 / STT_SAMPLE_RATE as f32).sin()).collect::<Vec<_>>();
        let peak = |s: Vec<f32>| s.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0;
        let low = peak(spectrum(&tone(150.0)));
        let high = peak(spectrum(&tone(2500.0)));
        assert!(low < 6, "150 Hz peaked at band {low}");
        assert!(high > 18, "2.5 kHz peaked at band {high}");
    }

    #[test]
    fn level_scale() {
        assert_eq!(level(&[]), 0.0);
        assert_eq!(level(&[0.0; 100]), 0.0);
        assert!(level(&[1.0; 100]) > 0.99);
        let quiet = level(&[0.01; 100]);
        assert!(quiet > 0.2 && quiet < 0.5);
    }
}
