//! Audio playback: a queue of PCM clips played on one output stream owned by a
//! dedicated thread. Supports stop (clears queue), volume and device changes.

use super::devices;
use crate::errors::{AppError, AppResult};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct Clip {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// Opaque tag so callers can cancel clips of one utterance/turn.
    pub tag: String,
}

#[derive(Default)]
struct Shared {
    queue: VecDeque<Vec<f32>>, // already resampled to device rate, mono
    tags: VecDeque<String>,
    current: Vec<f32>,
    current_tag: Option<String>,
    pos: usize,
}

enum Cmd {
    Open(Option<String>),
    Shutdown,
}

pub struct Player {
    shared: Arc<Mutex<Shared>>,
    volume_bits: Arc<AtomicU32>,
    device_rate: Arc<AtomicU32>,
    playing: Arc<AtomicBool>,
    cmd: Sender<Cmd>,
}

impl Player {
    pub fn new(device_id: Option<String>) -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let volume_bits = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let device_rate = Arc::new(AtomicU32::new(0));
        let playing = Arc::new(AtomicBool::new(false));
        let (cmd, rx) = mpsc::channel();
        let (s, v, r, p) = (shared.clone(), volume_bits.clone(), device_rate.clone(), playing.clone());
        std::thread::Builder::new()
            .name("audio-playback".into())
            .spawn(move || run(rx, s, v, r, p))
            .expect("spawn playback thread");
        let player = Self { shared, volume_bits, device_rate, playing, cmd };
        let _ = player.cmd.send(Cmd::Open(device_id));
        player
    }

    pub fn set_device(&self, device_id: Option<String>) {
        let _ = self.cmd.send(Cmd::Open(device_id));
    }

    pub fn set_volume(&self, v: f32) {
        self.volume_bits.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn enqueue(&self, clip: Clip) -> AppResult<()> {
        let rate = self.device_rate.load(Ordering::Relaxed);
        if rate == 0 {
            return Err(AppError::Audio("audio output is not available".into()));
        }
        let samples = if clip.sample_rate == rate {
            clip.samples
        } else {
            sherpa_onnx::LinearResampler::create(clip.sample_rate as i32, rate as i32)
                .map(|r| r.resample(&clip.samples, true))
                .ok_or_else(|| AppError::Audio("resampler unavailable".into()))?
        };
        let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
        s.queue.push_back(samples);
        s.tags.push_back(clip.tag);
        Ok(())
    }

    /// Stops everything, or only clips with `tag`.
    pub fn stop(&self, tag: Option<&str>) {
        let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
        match tag {
            None => {
                s.queue.clear();
                s.tags.clear();
                s.current.clear();
                s.current_tag = None;
                s.pos = 0;
            }
            Some(tag) => {
                let queue: Vec<Vec<f32>> = s.queue.drain(..).collect();
                let tags: Vec<String> = s.tags.drain(..).collect();
                let keep: Vec<(Vec<f32>, String)> = queue.into_iter().zip(tags).filter(|(_, t)| t != tag).collect();
                for (q, t) in keep {
                    s.queue.push_back(q);
                    s.tags.push_back(t);
                }
                if s.current_tag.as_deref() == Some(tag) {
                    s.current.clear();
                    s.current_tag = None;
                    s.pos = 0;
                }
            }
        }
    }

    /// True while audio is queued or playing.
    pub fn is_active(&self) -> bool {
        let s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
        !s.queue.is_empty() || s.pos < s.current.len() || self.playing.load(Ordering::Relaxed)
    }

    pub fn is_available(&self) -> bool {
        self.device_rate.load(Ordering::Relaxed) != 0
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.cmd.send(Cmd::Shutdown);
    }
}

fn run(rx: Receiver<Cmd>, shared: Arc<Mutex<Shared>>, volume: Arc<AtomicU32>, rate: Arc<AtomicU32>, playing: Arc<AtomicBool>) {
    let mut stream: Option<cpal::Stream> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Cmd::Open(id)) => {
                drop(stream.take());
                rate.store(0, Ordering::Relaxed);
                match open(id.as_deref(), shared.clone(), volume.clone(), playing.clone()) {
                    Ok((s, r)) => {
                        rate.store(r, Ordering::Relaxed);
                        stream = Some(s);
                    }
                    Err(e) => tracing::warn!(error = %e, "audio output unavailable"),
                }
            }
            Ok(Cmd::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn open(id: Option<&str>, shared: Arc<Mutex<Shared>>, volume: Arc<AtomicU32>, playing: Arc<AtomicBool>) -> AppResult<(cpal::Stream, u32)> {
    let device = devices::output_device(id)?;
    let supported = device.default_output_config().map_err(|e| AppError::Audio(e.to_string()))?;
    let config = supported.config();
    let r = config.sample_rate;
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, shared, volume, playing),
        SampleFormat::I16 => build::<i16>(&device, config, shared, volume, playing),
        SampleFormat::U16 => build::<u16>(&device, config, shared, volume, playing),
        SampleFormat::I32 => build::<i32>(&device, config, shared, volume, playing),
        other => Err(format!("unsupported output format {other:?}")),
    }
    .map_err(AppError::Audio)?;
    stream.play().map_err(|e| AppError::Audio(e.to_string()))?;
    Ok((stream, r))
}

fn build<T>(device: &cpal::Device, config: cpal::StreamConfig, shared: Arc<Mutex<Shared>>, volume: Arc<AtomicU32>, playing: Arc<AtomicBool>) -> Result<cpal::Stream, String>
where
    T: SizedSample + cpal::FromSample<f32> + Send + 'static,
{
    let channels = config.channels.max(1) as usize;
    device
        .build_output_stream::<T, _, _>(
            config,
            move |out: &mut [T], _| {
                let vol = f32::from_bits(volume.load(Ordering::Relaxed));
                let mut s = shared.lock().unwrap_or_else(|p| p.into_inner());
                let mut any = false;
                for frame in out.chunks_mut(channels) {
                    if s.pos >= s.current.len() {
                        match s.queue.pop_front() {
                            Some(next) => {
                                s.current = next;
                                s.current_tag = s.tags.pop_front();
                                s.pos = 0;
                            }
                            None => {
                                s.current.clear();
                                s.current_tag = None;
                                s.pos = 0;
                                break;
                            }
                        }
                    }
                    let v = s.current.get(s.pos).copied().unwrap_or(0.0) * vol;
                    s.pos += 1;
                    any = true;
                    for sample in frame.iter_mut() {
                        *sample = T::from_sample_(v);
                    }
                }
                playing.store(any, Ordering::Relaxed);
            },
            |e| tracing::warn!(error = %e, "playback stream error"),
            Some(Duration::from_secs(3)),
        )
        .map_err(|e| e.to_string())
}
