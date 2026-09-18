//! Optional GPU (CUDA) acceleration for the local speech models.
//!
//! The app ships the CPU build of ONNX Runtime, because the CUDA build is
//! ~570 MB and only useful on NVIDIA cards. Users who want it download a "GPU
//! pack" into the app data folder.
//!
//! Windows resolves a program's imported DLLs from the folder the executable
//! sits in, before `main` runs and before anything on `PATH`. So the pack
//! cannot be loaded into the running process, and putting it on `PATH` would
//! change nothing. Instead the executable is copied into the pack folder and
//! restarted from there: its neighbours are then the CUDA libraries, and the
//! same binary runs with the GPU build of sherpa-onnx.

use crate::errors::{AppError, AppResult};
use crate::services::hardware::HardwareInfo;
use crate::services::models::download::extract_tar_bz2;
use futures_util::StreamExt;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

/// Matches the `sherpa-onnx` crate version this app links against: the pack
/// replaces those libraries, so the versions have to agree.
pub const PACK_VERSION: &str = "1.13.8";
const PACK_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-cuda-12.x-cudnn-9.x-onnxruntime1.28.2-win-x64-cuda.tar.bz2";
/// CUDA runtime libraries the ONNX Runtime CUDA provider imports. They come
/// from NVIDIA's redistributable archives, so the user does not have to install
/// the CUDA toolkit: without them the provider fails to load and takes the
/// process down with it.
const CUDA_PARTS: [Part; 2] = [
    Part {
        url: "https://developer.download.nvidia.com/compute/cuda/redist/cuda_cudart/windows-x86_64/cuda_cudart-windows-x86_64-12.6.77-archive.zip",
        bytes: 2_500_000,
        wanted: &["cudart64_12.dll"],
    },
    Part {
        url: "https://developer.download.nvidia.com/compute/cuda/redist/libcublas/windows-x86_64/libcublas-windows-x86_64-12.6.4.1-archive.zip",
        bytes: 428_000_000,
        wanted: &["cublas64_12.dll", "cublasLt64_12.dll"],
    },
];
/// Rough download size of every part, for the progress bar.
pub const PACK_BYTES: u64 = 595_000_000 + 2_500_000 + 428_000_000;

/// One zipped NVIDIA archive and the libraries taken out of it.
struct Part {
    url: &'static str,
    /// Download size, counted into `PACK_BYTES` for the progress bar.
    #[allow(dead_code)]
    bytes: u64,
    wanted: &'static [&'static str],
}
/// Set on the relaunched process so it does not relaunch itself again.
const RELAUNCH_MARKER: &str = "LOCAL_ASSISTANT_GPU_ACTIVE";
/// Files that must be present for the pack to count as installed.
const REQUIRED: [&str; 6] = [
    "onnxruntime.dll",
    "onnxruntime_providers_cuda.dll",
    "sherpa-onnx-c-api.dll",
    "cudart64_12.dll",
    "cublas64_12.dll",
    "cublasLt64_12.dll",
];

static GPU_ACTIVE: AtomicBool = AtomicBool::new(false);

/// The ONNX Runtime provider the speech models should be created with.
pub fn provider() -> &'static str {
    if GPU_ACTIVE.load(Ordering::Relaxed) {
        "cuda"
    } else {
        "cpu"
    }
}

/// True when this process is running with the GPU pack's libraries loaded.
pub fn is_active() -> bool {
    GPU_ACTIVE.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuStatus {
    /// An NVIDIA card was detected.
    pub supported: bool,
    pub gpu_name: Option<String>,
    pub installed: bool,
    /// Size of the installed pack on disk.
    pub size_bytes: u64,
    /// Download size of the pack.
    pub download_bytes: u64,
    /// The models in this process are running on the GPU.
    pub active: bool,
    /// Installed and enabled, but the app has to restart to use it.
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuProgress {
    /// "downloading" | "extracting" | "done" | "error" | "cancelled"
    pub state: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub error: Option<String>,
}

/// The app data folder, resolved without Tauri so the GPU switch can happen
/// before the app (and its single-instance guard) starts.
pub fn default_data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("com.localassistant.app"))
}

/// Whether the user turned GPU acceleration on. It lives in a file rather than
/// in the settings database because it is read before the database is opened.
pub fn is_enabled(data_dir: &Path) -> bool {
    pack_dir(data_dir).with_file_name("enabled").exists()
}

pub fn set_enabled(data_dir: &Path, enabled: bool) -> AppResult<()> {
    let flag = pack_dir(data_dir).with_file_name("enabled");
    if enabled {
        if let Some(parent) = flag.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&flag, b"1")?;
    } else if flag.exists() {
        std::fs::remove_file(&flag)?;
    }
    Ok(())
}

/// Called first thing at startup: relaunches this process with the GPU pack's
/// libraries when the pack is installed and switched on. Returns true when the
/// caller should exit because the replacement process is running.
pub fn activate_early() -> bool {
    let Some(dir) = default_data_dir() else { return false };
    let enabled = is_enabled(&dir);
    activate(&dir, enabled)
}

pub fn pack_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("gpu").join(format!("cuda-{PACK_VERSION}"))
}

pub fn is_installed(data_dir: &Path) -> bool {
    let dir = pack_dir(data_dir);
    REQUIRED.iter().all(|f| dir.join(f).is_file())
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .filter_map(|e| e.ok())
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

pub fn status(data_dir: &Path, hw: &HardwareInfo, enabled: bool) -> GpuStatus {
    let installed = is_installed(data_dir);
    let active = is_active();
    GpuStatus {
        supported: hw.has_nvidia(),
        gpu_name: hw.gpus.iter().find(|g| g.vendor == "nvidia").map(|g| g.name.clone()),
        installed,
        size_bytes: if installed { dir_size(&pack_dir(data_dir)) } else { 0 },
        download_bytes: PACK_BYTES,
        active,
        restart_required: installed && enabled && !active,
    }
}

/// Restarts the app from inside the pack folder, where the CUDA libraries are
/// its nearest neighbours. Returns true when the replacement process started
/// and this one should exit.
pub fn activate(data_dir: &Path, enabled: bool) -> bool {
    if std::env::var_os(RELAUNCH_MARKER).is_some() {
        // This is the copy running inside the pack: it already loaded the CUDA
        // libraries, because Windows looked in its own folder first.
        GPU_ACTIVE.store(true, Ordering::Relaxed);
        tracing::info!("speech models are running on the GPU");
        return false;
    }
    if !enabled || !is_installed(data_dir) {
        return false;
    }
    let dir = pack_dir(data_dir);
    let copy = match stage_executable(&dir) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "could not place the app in the GPU pack folder, staying on the CPU");
            return false;
        }
    };
    match std::process::Command::new(&copy)
        .args(std::env::args_os().skip(1))
        .env(RELAUNCH_MARKER, "1")
        .spawn()
    {
        Ok(_) => {
            tracing::info!(exe = %copy.display(), "restarting with the GPU pack");
            true
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not restart with the GPU pack, staying on the CPU");
            false
        }
    }
}

/// Copies this executable into the pack folder, refreshing an older copy so an
/// updated app never runs as a stale binary.
fn stage_executable(pack: &Path) -> AppResult<PathBuf> {
    let exe = std::env::current_exe()?;
    let name = exe.file_name().ok_or_else(|| AppError::Other("the app has no file name".into()))?;
    let copy = pack.join(name);
    let fresh = match (std::fs::metadata(&exe), std::fs::metadata(&copy)) {
        (Ok(a), Ok(b)) => a.len() == b.len() && a.modified().ok() == b.modified().ok(),
        _ => false,
    };
    if !fresh {
        std::fs::copy(&exe, &copy)?;
    }
    Ok(copy)
}

pub fn remove(data_dir: &Path) -> AppResult<()> {
    let dir = pack_dir(data_dir);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// Downloads and extracts the GPU pack into the app data folder.
pub async fn install(
    data_dir: &Path,
    cancel: CancellationToken,
    on_progress: &(dyn Fn(GpuProgress) + Send + Sync),
) -> AppResult<()> {
    let dir = pack_dir(data_dir);
    // A sibling folder, not `with_extension`, which would eat the patch number.
    let staging = dir.with_file_name(format!("cuda-{PACK_VERSION}-staging"));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;

    let progress = |state: &str, downloaded: u64, total: u64, error: Option<String>| {
        on_progress(GpuProgress { state: state.into(), downloaded_bytes: downloaded, total_bytes: total, error })
    };

    let result: AppResult<()> = async {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .user_agent(concat!("LocalAssistant/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| AppError::Download(e.to_string()))?;
        let resp = client.get(PACK_URL).send().await.map_err(|e| AppError::Download(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::Download(format!("{PACK_URL} returned {}", resp.status())));
        }
        let total = resp.content_length().unwrap_or(PACK_BYTES);
        let archive = staging.join("pack.tar.bz2");
        let mut out = std::fs::File::create(&archive)?;
        let mut stream = resp.bytes_stream();
        let mut got = 0u64;
        let mut last = Instant::now();
        loop {
            let next = tokio::select! {
                n = stream.next() => n,
                _ = cancel.cancelled() => return Err(AppError::Cancelled),
            };
            let Some(chunk) = next else { break };
            let chunk = chunk.map_err(|e| AppError::Download(e.to_string()))?;
            out.write_all(&chunk)?;
            got += chunk.len() as u64;
            if last.elapsed() > Duration::from_millis(250) {
                progress("downloading", got, total, None);
                last = Instant::now();
            }
        }
        out.flush()?;
        drop(out);

        progress("extracting", got, total, None);
        let (src, dest) = (archive.clone(), staging.clone());
        tokio::task::spawn_blocking(move || extract_tar_bz2(&src, &dest))
            .await
            .map_err(|e| AppError::Download(e.to_string()))??;
        let _ = std::fs::remove_file(&archive);
        flatten_libraries(&staging)?;

        let mut done = got;
        for part in CUDA_PARTS {
            let zip_path = staging.join("part.zip");
            let mut resp = client.get(part.url).send().await.map_err(|e| AppError::Download(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(AppError::Download(format!("{} returned {}", part.url, resp.status())));
            }
            let mut out = std::fs::File::create(&zip_path)?;
            loop {
                let next = tokio::select! {
                    c = resp.chunk() => c.map_err(|e| AppError::Download(e.to_string()))?,
                    _ = cancel.cancelled() => return Err(AppError::Cancelled),
                };
                let Some(chunk) = next else { break };
                out.write_all(&chunk)?;
                done += chunk.len() as u64;
                if last.elapsed() > Duration::from_millis(250) {
                    progress("downloading", done, PACK_BYTES, None);
                    last = Instant::now();
                }
            }
            out.flush()?;
            drop(out);
            progress("extracting", done, PACK_BYTES, None);
            let (zip_path2, dest, wanted) = (zip_path.clone(), staging.clone(), part.wanted);
            tokio::task::spawn_blocking(move || extract_zip_libraries(&zip_path2, &dest, wanted))
                .await
                .map_err(|e| AppError::Download(e.to_string()))??;
            let _ = std::fs::remove_file(&zip_path);
        }
        let missing: Vec<&str> = REQUIRED.iter().copied().filter(|f| !staging.join(f).is_file()).collect();
        if !missing.is_empty() {
            return Err(AppError::Download(format!("the GPU pack is missing {}", missing.join(", "))));
        }
        Ok(())
    }
    .await;

    match result {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&dir);
            if let Some(parent) = dir.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&staging, &dir)?;
            tracing::info!(dir = %dir.display(), "GPU pack installed");
            progress("done", PACK_BYTES, PACK_BYTES, None);
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            if matches!(e, AppError::Cancelled) {
                progress("cancelled", 0, PACK_BYTES, None);
            } else {
                tracing::warn!(error = %e, "GPU pack download failed");
                progress("error", 0, PACK_BYTES, Some(e.to_string()));
            }
            Err(e)
        }
    }
}

/// Pulls the named libraries out of an NVIDIA archive, wherever they sit in it,
/// and drops them in the pack root next to everything else.
fn extract_zip_libraries(archive: &Path, dest: &Path, wanted: &[&str]) -> AppResult<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| AppError::Download(e.to_string()))?;
    let mut found = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| AppError::Download(e.to_string()))?;
        let Some(name) = entry.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())) else {
            continue;
        };
        if !wanted.iter().any(|w| w.eq_ignore_ascii_case(&name)) {
            continue;
        }
        let mut out = std::fs::File::create(dest.join(&name))?;
        std::io::copy(&mut entry, &mut out)?;
        found.push(name);
    }
    let missing: Vec<&str> = wanted.iter().copied().filter(|w| !found.iter().any(|f| f.eq_ignore_ascii_case(w))).collect();
    if !missing.is_empty() {
        return Err(AppError::Download(format!("{} does not contain {}", archive.display(), missing.join(", "))));
    }
    Ok(())
}

/// The archive keeps its libraries in `lib/` and `bin/`; the DLL search path
/// only looks at one folder, so every library is moved to the pack root.
fn flatten_libraries(root: &Path) -> AppResult<()> {
    for sub in ["lib", "bin"] {
        let dir = root.join(sub);
        if !dir.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("dll")) != Some(true) {
                continue;
            }
            let target = root.join(entry.file_name());
            if target.exists() {
                continue;
            }
            std::fs::rename(&path, &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_needs_all_its_libraries() {
        let dir = tempfile::tempdir().unwrap();
        let pack = pack_dir(dir.path());
        std::fs::create_dir_all(&pack).unwrap();
        assert!(!is_installed(dir.path()));
        for f in REQUIRED {
            std::fs::write(pack.join(f), b"x").unwrap();
        }
        assert!(is_installed(dir.path()));
        std::fs::remove_file(pack.join(REQUIRED[1])).unwrap();
        assert!(!is_installed(dir.path()), "a half-extracted pack must not count as installed");
    }

    #[test]
    fn libraries_move_next_to_the_pack_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(root.join("lib/onnxruntime.dll"), b"x").unwrap();
        std::fs::write(root.join("bin/cudnn64_9.dll"), b"x").unwrap();
        std::fs::write(root.join("lib/notes.txt"), b"x").unwrap();
        flatten_libraries(root).unwrap();
        assert!(root.join("onnxruntime.dll").is_file());
        assert!(root.join("cudnn64_9.dll").is_file());
        assert!(root.join("lib/notes.txt").is_file(), "only libraries move");
    }

    #[test]
    fn the_executable_is_staged_once_and_refreshed_when_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("pack");
        std::fs::create_dir_all(&pack).unwrap();
        let exe = std::env::current_exe().unwrap();
        let copy = stage_executable(&pack).unwrap();
        assert_eq!(copy, pack.join(exe.file_name().unwrap()));
        assert_eq!(std::fs::metadata(&copy).unwrap().len(), std::fs::metadata(&exe).unwrap().len());

        // A stale copy of a different size is replaced.
        std::fs::write(&copy, b"old build").unwrap();
        stage_executable(&pack).unwrap();
        assert_eq!(std::fs::metadata(&copy).unwrap().len(), std::fs::metadata(&exe).unwrap().len());
    }

    #[test]
    fn provider_is_cpu_until_the_pack_is_active() {
        assert_eq!(provider(), "cpu");
    }
}
