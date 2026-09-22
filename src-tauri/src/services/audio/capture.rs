//! Microphone capture on a dedicated thread. Audio is downmixed to mono,
//! resampled to 16 kHz and delivered in chunks through a channel.

use super::{devices, level, spectrum, HighPassFilter, STT_SAMPLE_RATE};
use crate::errors::{AppError, AppResult};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

pub enum CaptureEvent {
    /// 16 kHz mono samples.
    Samples(Vec<f32>),
    /// Meter value 0..1 and voice spectrum bands (about 20 per second).
    Level(f32, Vec<f32>),
    Error(String),
}

pub struct Capture {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub device_name: String,
}

impl Capture {
    pub fn start(device_id: Option<&str>, mic_only: bool) -> AppResult<(Capture, Receiver<CaptureEvent>)> {
        let device = devices::input_device(device_id, mic_only)?;
        let device_name = device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| device.to_string());
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let stop2 = stop.clone();
        let thread = std::thread::Builder::new()
            .name("mic-capture".into())
            .spawn(move || {
                match build_stream(&device, tx.clone()) {
                    Ok(stream) => {
                        if let Err(e) = stream.play() {
                            let _ = ready_tx.send(Err(e.to_string()));
                            return;
                        }
                        let _ = ready_tx.send(Ok(()));
                        while !stop2.load(Ordering::Relaxed) {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        drop(stream);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })
            .map_err(|e| AppError::Audio(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok((Capture { stop, thread: Some(thread), device_name }, rx)),
            Ok(Err(e)) => Err(AppError::Audio(format!("could not open microphone: {e}"))),
            Err(_) => Err(AppError::Audio("microphone did not start in time".into())),
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Watches the default speaker/output device via WASAPI loopback and
/// reports how loud it currently is, so a microphone session can ignore
/// audio it picked up from the speakers instead of the user's voice.
/// (Windows only: `Device::build_input_stream` transparently loopbacks
/// when the device is an output device, per cpal's WASAPI backend.)
pub struct LoopbackMonitor {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    level_bits: Arc<AtomicU32>,
}

impl LoopbackMonitor {
    /// `device_id` is the output the assistant plays through (the same device
    /// the TTS player uses); `None` watches the system default output.
    pub fn start(device_id: Option<&str>) -> AppResult<Self> {
        let device = devices::output_device(device_id)?;
        tracing::info!(
            device = %device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| device.to_string()),
            "system audio monitor opened"
        );
        let level_bits = Arc::new(AtomicU32::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (level2, stop2) = (level_bits.clone(), stop.clone());
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let thread = std::thread::Builder::new()
            .name("system-audio-monitor".into())
            .spawn(move || match build_loopback_stream(&device, level2) {
                Ok(stream) => {
                    if let Err(e) = stream.play() {
                        let _ = ready_tx.send(Err(e.to_string()));
                        return;
                    }
                    let _ = ready_tx.send(Ok(()));
                    while !stop2.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            })
            .map_err(|e| AppError::Audio(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(Self { stop, thread: Some(thread), level_bits }),
            Ok(Err(e)) => Err(AppError::Audio(format!("could not listen to system audio: {e}"))),
            Err(_) => Err(AppError::Audio("system audio monitor did not start in time".into())),
        }
    }

    /// Current 0..1 level of whatever is playing through the speakers.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level_bits.load(Ordering::Relaxed))
    }

    /// A cheap, cloneable handle to the live level, readable from another
    /// thread without holding onto the monitor (and its stream) itself.
    pub fn level_handle(&self) -> Arc<AtomicU32> {
        self.level_bits.clone()
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for LoopbackMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

fn build_loopback_stream(device: &cpal::Device, level_bits: Arc<AtomicU32>) -> Result<cpal::Stream, String> {
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    match supported.sample_format() {
        SampleFormat::F32 => build_level_only::<f32>(device, config, level_bits),
        SampleFormat::I16 => build_level_only::<i16>(device, config, level_bits),
        SampleFormat::U16 => build_level_only::<u16>(device, config, level_bits),
        SampleFormat::I32 => build_level_only::<i32>(device, config, level_bits),
        SampleFormat::U8 => build_level_only::<u8>(device, config, level_bits),
        other => Err(format!("unsupported system audio format {other:?}")),
    }
}

fn build_level_only<T>(device: &cpal::Device, config: cpal::StreamConfig, level_bits: Arc<AtomicU32>) -> Result<cpal::Stream, String>
where
    T: SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                let mono: Vec<f32> = data.chunks(channels).map(|frame| frame.iter().map(|s| <f32 as cpal::FromSample<T>>::from_sample_(*s)).sum::<f32>() / channels as f32).collect();
                level_bits.store(level(&mono).to_bits(), Ordering::Relaxed);
            },
            |_e| {},
            Some(Duration::from_secs(3)),
        )
        .map_err(|e| e.to_string())
}

fn build_stream(device: &cpal::Device, tx: Sender<CaptureEvent>) -> Result<cpal::Stream, String> {
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    tracing::info!(
        device = %device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| device.to_string()),
        sample_rate = config.sample_rate,
        channels = config.channels,
        format = ?supported.sample_format(),
        "microphone opened"
    );
    match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(device, config, tx),
        SampleFormat::I16 => build::<i16>(device, config, tx),
        SampleFormat::U16 => build::<u16>(device, config, tx),
        SampleFormat::I32 => build::<i32>(device, config, tx),
        SampleFormat::U8 => build::<u8>(device, config, tx),
        other => Err(format!("unsupported microphone sample format {other:?}")),
    }
}

fn build<T>(device: &cpal::Device, config: cpal::StreamConfig, tx: Sender<CaptureEvent>) -> Result<cpal::Stream, String>
where
    T: SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let rate = config.sample_rate;
    let resampler = if rate != STT_SAMPLE_RATE {
        Some(sherpa_onnx::LinearResampler::create(rate as i32, STT_SAMPLE_RATE as i32).ok_or("resampler")?)
    } else {
        None
    };
    let mut meter_buf: Vec<f32> = Vec::new();
    let mut ticks: u64 = 0;
    let meter_every = (STT_SAMPLE_RATE / 20) as usize;
    let err_tx = tx.clone();
    // Cuts fan/AC hum and room rumble, which sit well below speech's
    // fundamental frequency, before the audio reaches the meter, VAD or
    // recognizer.
    let mut hp = HighPassFilter::new(100.0, STT_SAMPLE_RATE);
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                let mono: Vec<f32> = data
                    .chunks(channels)
                    .map(|frame| frame.iter().map(|s| <f32 as cpal::FromSample<T>>::from_sample_(*s)).sum::<f32>() / channels as f32)
                    .collect();
                let mut out = match &resampler {
                    Some(r) => r.resample(&mono, false),
                    None => mono,
                };
                if out.is_empty() {
                    return;
                }
                hp.process(&mut out);
                meter_buf.extend_from_slice(&out);
                if meter_buf.len() >= meter_every {
                    let lvl = level(&meter_buf);
                    // One line per second of audio: enough to debug a silent microphone.
                    ticks += 1;
                    if ticks % 20 == 0 {
                        tracing::debug!(level = lvl, "microphone level");
                    }
                    let _ = tx.send(CaptureEvent::Level(lvl, spectrum(&meter_buf)));
                    meter_buf.clear();
                }
                let _ = tx.send(CaptureEvent::Samples(out));
            },
            move |e| {
                let _ = err_tx.send(CaptureEvent::Error(e.to_string()));
            },
            Some(Duration::from_secs(3)),
        )
        .map_err(|e| e.to_string())
}
