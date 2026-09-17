//! Consent-gated model downloads with SHA-256 verification and archive extraction.

use super::catalog::CatalogModel;
use super::{ModelStore, COMPLETE_MARKER};
use crate::errors::{AppError, AppResult};
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model_id: String,
    /// "downloading" | "verifying" | "extracting" | "done" | "error" | "cancelled"
    pub state: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub error: Option<String>,
}

pub async fn install(
    store: &ModelStore,
    model: &CatalogModel,
    cancel: CancellationToken,
    on_progress: &(dyn Fn(DownloadProgress) + Send + Sync),
) -> AppResult<()> {
    let dir = store.model_dir(model.id);
    let staging = store.dir.join(format!(".staging-{}", model.id));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;

    let total = model.download_size();
    let mut done_bytes = 0u64;
    let progress = |state: &str, downloaded: u64, error: Option<String>| {
        on_progress(DownloadProgress {
            model_id: model.id.into(),
            state: state.into(),
            downloaded_bytes: downloaded,
            total_bytes: total,
            error,
        })
    };

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .user_agent(concat!("LocalAssistant/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| AppError::Download(e.to_string()))?;

    let result: AppResult<()> = async {
        for file in model.files {
            let tmp = staging.join(format!("download-{}.part", done_bytes));
            let resp = client.get(file.url).send().await.map_err(|e| AppError::Download(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(AppError::Download(format!("{} returned {}", file.url, resp.status())));
            }
            let mut out = std::fs::File::create(&tmp)?;
            let mut hasher = Sha256::new();
            let mut stream = resp.bytes_stream();
            let mut file_bytes = 0u64;
            let mut last_emit = Instant::now();
            loop {
                let next = tokio::select! {
                    n = stream.next() => n,
                    _ = cancel.cancelled() => return Err(AppError::Cancelled),
                };
                let Some(chunk) = next else { break };
                let chunk = chunk.map_err(|e| AppError::Download(e.to_string()))?;
                out.write_all(&chunk)?;
                hasher.update(&chunk);
                file_bytes += chunk.len() as u64;
                if last_emit.elapsed() > Duration::from_millis(250) {
                    progress("downloading", done_bytes + file_bytes, None);
                    last_emit = Instant::now();
                }
            }
            out.flush()?;
            drop(out);
            done_bytes += file_bytes;

            progress("verifying", done_bytes, None);
            if let Some(expected) = file.sha256 {
                let actual = hex::encode(hasher.finalize());
                if !actual.eq_ignore_ascii_case(expected) {
                    return Err(AppError::Download(format!("checksum mismatch for {}", file.url)));
                }
            }

            if file.archive {
                progress("extracting", done_bytes, None);
                let tmp2 = tmp.clone();
                let dest = staging.clone();
                tokio::task::spawn_blocking(move || extract_tar_bz2(&tmp2, &dest))
                    .await
                    .map_err(|e| AppError::Download(e.to_string()))??;
                let _ = std::fs::remove_file(&tmp);
            } else {
                let dest = safe_join(&staging, Path::new(file.dest))?;
                std::fs::rename(&tmp, dest)?;
            }
        }
        Ok(())
    }
    .await;

    match result {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::rename(&staging, &dir)?;
            std::fs::write(
                dir.join(COMPLETE_MARKER),
                serde_json::json!({"id": model.id, "installedAt": chrono::Utc::now().to_rfc3339()}).to_string(),
            )?;
            tracing::info!(model = model.id, bytes = done_bytes, "model installed");
            progress("done", total, None);
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            if matches!(e, AppError::Cancelled) {
                progress("cancelled", done_bytes, None);
            } else {
                tracing::warn!(model = model.id, error = %e, "model download failed");
                progress("error", done_bytes, Some(e.to_string()));
            }
            Err(e)
        }
    }
}

/// Joins a relative archive path onto `root`, rejecting absolute paths and `..`.
fn safe_join(root: &Path, rel: &Path) -> AppResult<PathBuf> {
    let mut out = root.to_path_buf();
    for c in rel.components() {
        match c {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            _ => return Err(AppError::Download(format!("unsafe path in archive: {}", rel.display()))),
        }
    }
    Ok(out)
}

/// Extracts a .tar.bz2 into `dest`, stripping the archive's top-level folder.
pub fn extract_tar_bz2(archive: &Path, dest: &Path) -> AppResult<()> {
    let f = std::fs::File::open(archive)?;
    let mut tar = tar::Archive::new(bzip2::read::BzDecoder::new(std::io::BufReader::new(f)));
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let stripped: PathBuf = path.components().skip(1).collect();
        if stripped.as_os_str().is_empty() {
            continue;
        }
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            continue; // skip links and special files
        }
        let target = safe_join(dest, &stripped)?;
        if kind.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            entry.unpack(&target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_strips_top_folder() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.tar.bz2");
        {
            let f = std::fs::File::create(&archive).unwrap();
            let enc = bzip2::write::BzEncoder::new(f, bzip2::Compression::fast());
            let mut b = tar::Builder::new(enc);
            let data = b"hello";
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            b.append_data(&mut h, "vits-piper-x/espeak-ng-data/voices.txt", &data[..]).unwrap();
            let mut h2 = tar::Header::new_gnu();
            h2.set_size(data.len() as u64);
            h2.set_cksum();
            b.append_data(&mut h2, "vits-piper-x/model.onnx", &data[..]).unwrap();
            b.into_inner().unwrap().finish().unwrap();
        }
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        extract_tar_bz2(&archive, &out).unwrap();
        assert_eq!(std::fs::read(out.join("model.onnx")).unwrap(), b"hello");
        assert!(out.join("espeak-ng-data").join("voices.txt").exists());
    }

    #[test]
    fn rejects_traversal() {
        let root = Path::new("C:/models");
        assert!(safe_join(root, Path::new("../x")).is_err());
        assert!(safe_join(root, Path::new("a/b.onnx")).is_ok());
    }
}
