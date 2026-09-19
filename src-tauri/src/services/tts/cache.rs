//! On-disk cache of generated speech.
//!
//! Orpheus sentences cost seconds of GPU time, so the same sentence spoken
//! again (replay, a repeated answer) is read from a WAV file instead. Files are
//! named after the model, voice and text, and the oldest are dropped once the
//! cache passes its size limit.

use crate::errors::AppResult;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    pub files: u32,
    pub bytes: u64,
}

pub struct AudioCache {
    dir: PathBuf,
}

impl AudioCache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn path(&self, key: &str) -> PathBuf {
        let digest = Sha256::digest(key.as_bytes());
        self.dir.join(format!("{}.wav", hex::encode(&digest[..16])))
    }

    /// Cached audio for `key`, if it was stored before.
    pub fn get(&self, key: &str) -> Option<(Vec<f32>, u32)> {
        let path = self.path(key);
        let bytes = std::fs::read(&path).ok()?;
        let audio = read_wav(&bytes);
        if audio.is_none() {
            // Truncated by a crash or a full disk; make room for a fresh one.
            let _ = std::fs::remove_file(&path);
        }
        audio
    }

    /// Stores `samples`, then trims the cache back under `max_bytes`.
    pub fn put(&self, key: &str, samples: &[f32], sample_rate: u32, max_bytes: u64) -> AppResult<()> {
        if max_bytes == 0 || samples.is_empty() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.path(key), write_wav(samples, sample_rate))?;
        self.trim(max_bytes);
        Ok(())
    }

    pub fn info(&self) -> CacheInfo {
        let files = self.files();
        CacheInfo { files: files.len() as u32, bytes: files.iter().map(|(_, size, _)| size).sum() }
    }

    pub fn clear(&self) {
        for (path, _, _) in self.files() {
            let _ = std::fs::remove_file(path);
        }
    }

    /// (path, size, modified) of every cached clip.
    fn files(&self) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "wav"))
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                Some((e.path(), meta.len(), meta.modified().unwrap_or(std::time::UNIX_EPOCH)))
            })
            .collect()
    }

    /// Deletes the oldest clips until the cache fits in `max_bytes`.
    fn trim(&self, max_bytes: u64) {
        let mut files = self.files();
        let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
        if total <= max_bytes {
            return;
        }
        files.sort_by_key(|(_, _, modified)| *modified);
        for (path, size, _) in files {
            if total <= max_bytes {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(size);
            }
        }
    }
}

/// Identifies one spoken sentence; a different voice or model is a different clip.
pub fn key(model: &str, voice: &str, text: &str) -> String {
    format!("v1|{model}|{voice}|{text}")
}

/// 16-bit mono PCM WAV, so the files can also be played by any audio player.
fn write_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let pcm: Vec<u8> = samples.iter().flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
    let mut wav = Vec::with_capacity(44 + pcm.len());
    for part in [
        &b"RIFF"[..],
        &(36 + pcm.len() as u32).to_le_bytes(),
        b"WAVEfmt ",
        &16u32.to_le_bytes(),
        &1u16.to_le_bytes(),
        &1u16.to_le_bytes(),
        &sample_rate.to_le_bytes(),
        &(sample_rate * 2).to_le_bytes(),
        &2u16.to_le_bytes(),
        &16u16.to_le_bytes(),
        b"data",
        &(pcm.len() as u32).to_le_bytes(),
        &pcm,
    ] {
        wav.extend_from_slice(part);
    }
    wav
}

fn read_wav(bytes: &[u8]) -> Option<(Vec<f32>, u32)> {
    if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let u32_at = |i: usize| -> Option<u32> { Some(u32::from_le_bytes(bytes.get(i..i + 4)?.try_into().ok()?)) };
    let sample_rate = u32_at(24)?;
    // Walk the chunks: some writers put extra chunks before the samples.
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let size = u32_at(pos + 4)? as usize;
        let start = pos + 8;
        if &bytes[pos..pos + 4] == b"data" {
            let end = start.saturating_add(size).min(bytes.len());
            let samples = bytes[start..end].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32767.0).collect();
            return Some((samples, sample_rate));
        }
        pos = start + size + (size % 2);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_trim() {
        let dir = tempfile::tempdir().unwrap();
        let cache = AudioCache::new(dir.path().to_path_buf());
        let samples: Vec<f32> = (0..2400).map(|i| (i as f32 / 50.0).sin() * 0.5).collect();
        assert!(cache.get(&key("m", "tara", "hello")).is_none());
        cache.put(&key("m", "tara", "hello"), &samples, 24_000, 10 << 20).unwrap();

        let (back, rate) = cache.get(&key("m", "tara", "hello")).expect("cached");
        assert_eq!(rate, 24_000);
        assert_eq!(back.len(), samples.len());
        assert!(back.iter().zip(&samples).all(|(a, b)| (a - b).abs() < 0.001));
        // A different voice or text is a separate clip.
        assert!(cache.get(&key("m", "leo", "hello")).is_none());
        assert_eq!(cache.info().files, 1);

        // A limit smaller than the stored clip empties the cache.
        cache.put(&key("m", "tara", "second"), &samples, 24_000, 1).unwrap();
        assert_eq!(cache.info().files, 0);
        cache.put(&key("m", "tara", "third"), &samples, 24_000, 10 << 20).unwrap();
        assert_eq!(cache.info().files, 1);
        cache.clear();
        assert_eq!(cache.info(), CacheInfo { files: 0, bytes: 0 });
    }

    #[test]
    fn rejects_damaged_file() {
        let dir = tempfile::tempdir().unwrap();
        let cache = AudioCache::new(dir.path().to_path_buf());
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(cache.path(&key("m", "tara", "x")), b"not a wav file at all, truncated").unwrap();
        assert!(cache.get(&key("m", "tara", "x")).is_none());
        assert_eq!(cache.info().files, 0, "a damaged file is deleted");
    }
}
