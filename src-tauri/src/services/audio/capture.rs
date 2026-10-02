//! Microphone capture on a dedicated thread. Audio is downmixed to mono,
//! resampled to 16 kHz and delivered in chunks through a channel.

use super::{devices, level, spectrum, HighPassFilter, STT_SAMPLE_RATE};
use crate::errors::{AppError, AppResult};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// A freshly started stream must stay alive this long before it is trusted.
/// Drivers (Realtek especially) can raise `AUDCLNT_E_BUFFER_ERROR` a few tens
/// of milliseconds into an otherwise valid stream, and WASAPI only lets a
/// client recover by rebuilding the stream, never by reusing the dead one.
const START_SETTLE: Duration = Duration::from_millis(250);
/// How many streams to build before giving up.
const START_ATTEMPTS: u32 = 4;
/// Pause between a failed attempt and the next one.
const RETRY_PAUSE: Duration = Duration::from_millis(300);

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
    pub fn start(device_id: Option<&str>) -> AppResult<(Capture, Receiver<CaptureEvent>)> {
        let device = devices::input_device(device_id)?;
        let device_name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| device.to_string());
        let (tx, rx) = mpsc::channel();
        let (err_tx, err_rx) = mpsc::channel::<String>();
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let stop2 = stop.clone();
        let thread = std::thread::Builder::new()
            .name("mic-capture".into())
            .spawn(move || {
                let mut last = "the microphone stream stopped immediately".to_string();
                for attempt in 1..=START_ATTEMPTS {
                    let built = match build_stream(&device, tx.clone(), err_tx.clone()) {
                        Ok(stream) => match stream.play() {
                            Ok(()) => Ok(stream),
                            Err(e) => {
                                last = e.to_string();
                                Err(e.to_string())
                            }
                        },
                        Err(e) => Err(e),
                    };
                    let stream = match built {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!(error = %e, attempt, "microphone stream could not be started");
                            std::thread::sleep(RETRY_PAUSE);
                            continue;
                        }
                    };
                    // The stream is playing; find out whether the driver keeps it.
                    let deadline = Instant::now() + START_SETTLE;
                    let mut died = None;
                    while Instant::now() < deadline {
                        std::thread::sleep(Duration::from_millis(20));
                        if let Ok(e) = err_rx.try_recv() {
                            died = Some(e);
                            break;
                        }
                    }
                    match died {
                        None => {
                            let _ = ready_tx.send(Ok(()));
                            // Healthy. Forward any later failure to the consumer.
                            while !stop2.load(Ordering::Relaxed) {
                                if let Ok(e) =
                                    err_rx.recv_timeout(Duration::from_millis(20))
                                {
                                    tracing::warn!(error = %e, "microphone stream failed");
                                    let _ = tx.send(CaptureEvent::Error(e));
                                }
                            }
                            drop(stream);
                            return;
                        }
                        Some(e) => {
                            // The stream is unusable; only a fresh one can recover.
                            tracing::warn!(
                                error = %e,
                                attempt,
                                "microphone stream died on start, rebuilding"
                            );
                            last = e;
                        }
                    }
                    drop(stream);
                    std::thread::sleep(RETRY_PAUSE);
                }
                let _ = ready_tx.send(Err(last));
            })
            .map_err(|e| AppError::Audio(e.to_string()))?;
        match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => Ok((
                Capture {
                    stop,
                    thread: Some(thread),
                    device_name,
                },
                rx,
            )),
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

fn build_stream(
    device: &cpal::Device,
    tx: Sender<CaptureEvent>,
    err_tx: Sender<String>,
) -> Result<cpal::Stream, String> {
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
        SampleFormat::F32 => build::<f32>(device, config, tx, err_tx.clone()),
        SampleFormat::I16 => build::<i16>(device, config, tx, err_tx.clone()),
        SampleFormat::U16 => build::<u16>(device, config, tx, err_tx.clone()),
        SampleFormat::I32 => build::<i32>(device, config, tx, err_tx.clone()),
        SampleFormat::U8 => build::<u8>(device, config, tx, err_tx.clone()),
        other => Err(format!("unsupported microphone sample format {other:?}")),
    }
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    tx: Sender<CaptureEvent>,
    err_tx: Sender<String>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + Send + 'static,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let rate = config.sample_rate;
    let resampler = if rate != STT_SAMPLE_RATE {
        Some(
            sherpa_onnx::LinearResampler::create(rate as i32, STT_SAMPLE_RATE as i32)
                .ok_or("resampler")?,
        )
    } else {
        None
    };
    let mut meter_buf: Vec<f32> = Vec::new();
    let mut ticks: u64 = 0;
    let meter_every = (STT_SAMPLE_RATE / 20) as usize;
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
                    .map(|frame| {
                        frame
                            .iter()
                            .map(|s| <f32 as cpal::FromSample<T>>::from_sample_(*s))
                            .sum::<f32>()
                            / channels as f32
                    })
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
                // Reported to the capture thread, which decides whether to
                // rebuild the stream or pass the failure on to the session.
                let _ = err_tx.send(e.to_string());
            },
            Some(Duration::from_secs(3)),
        )
        .map_err(|e| e.to_string())
}
