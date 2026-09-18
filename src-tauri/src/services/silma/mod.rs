//! SILMA TTS: natural Arabic speech from a local PyTorch model.
//!
//! SILMA (F5-TTS architecture) has no ONNX export, so it runs in a private
//! Python environment inside the app data folder, set up once with the
//! user's consent. The app starts the helper process itself when an Arabic
//! sentence needs it and talks to it over stdin/stdout ([`server.py`]); the
//! helper ends when the app does, because its stdin closes.
//!
//! Layout of `<data>/silma/`: `uv.exe` (Python installer), `python/`
//! (interpreter), `venv/` (PyTorch + SILMA), `weights/`, `hf/` (vocoder and
//! tashkeel models), `server.py`, `.complete`.

use crate::errors::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

const SERVER_PY: &str = include_str!("server.py");
const UV_URL: &str = "https://github.com/astral-sh/uv/releases/download/0.12.16/uv-x86_64-pc-windows-msvc.zip";
const MODEL_URL: &str = "https://huggingface.co/silma-ai/silma-tts/resolve/main/model.pt";
const VOCAB_URL: &str = "https://huggingface.co/silma-ai/silma-tts/resolve/main/vocab.txt";
const MODEL_BYTES: u64 = 2_603_209_272;
/// Rough total download: uv + Python + PyTorch (CUDA or CPU) + libraries + weights.
// On disk afterwards (GPU build): about 5.6 GB, mostly PyTorch's CUDA libraries.
const GPU_DOWNLOAD_BYTES: u64 = 18_000_000 + 30_000_000 + 3_000_000_000 + 600_000_000 + MODEL_BYTES;
const CPU_DOWNLOAD_BYTES: u64 = 18_000_000 + 30_000_000 + 250_000_000 + 600_000_000 + MODEL_BYTES;
/// SILMA's own dependencies minus NeMo text processing (needs pynini, which
/// has no Windows build; server.py stands in for it) and the Gradio demo UI.
const PACKAGES: &[&str] = &[
    "cached_path",
    "click",
    "ema_pytorch>=0.5.2",
    "hydra-core>=1.3.0",
    "librosa",
    "matplotlib",
    "numpy<=1.26.4",
    "pydub",
    "safetensors",
    "soundfile",
    "tomli",
    "torchdiffeq",
    "tqdm>=4.65.0",
    "transformers",
    "unidecode",
    "vocos",
    "x_transformers>=1.31.14",
    "catt_tashkeel==1.0.2",
    "huggingface_hub",
];
const SILMA_PACKAGE: &str = "silma-tts==1.0.5";
/// A sentence can take a while on a CPU; the first one also loads the model.
const LOAD_TIMEOUT: Duration = Duration::from_secs(240);
const SAY_TIMEOUT: Duration = Duration::from_secs(120);

pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("silma")
}

fn python(dir: &Path) -> PathBuf {
    dir.join("venv").join("Scripts").join("python.exe")
}

pub fn is_installed(data_dir: &Path) -> bool {
    let d = dir(data_dir);
    d.join(".complete").exists() && python(&d).exists()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilmaStatus {
    pub installed: bool,
    /// "off" | "starting" | "ready" | "failed"
    pub state: String,
    /// "cuda" | "cpu" once loaded.
    pub device: Option<String>,
    pub error: Option<String>,
    pub size_bytes: u64,
    pub download_bytes: u64,
    pub gpu: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SilmaProgress {
    /// "runtime" | "packages" | "weights" | "prepare" | "done" | "error" | "cancelled"
    pub stage: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    /// Latest line from the installer, for the curious.
    pub detail: Option<String>,
    pub error: Option<String>,
}

fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| match e.metadata() {
                    Ok(m) if m.is_dir() => dir_size(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                })
                .sum()
        })
        .unwrap_or(0)
}

// ---------------------------------------------------------------- install

async fn download(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: &CancellationToken,
    mut on_bytes: impl FnMut(u64),
) -> AppResult<()> {
    use futures_util::StreamExt;
    let resp = client.get(url).send().await.map_err(|e| AppError::Download(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(AppError::Download(format!("{url} returned {}", resp.status())));
    }
    let part = dest.with_extension("part");
    let mut out = std::fs::File::create(&part)?;
    let mut stream = resp.bytes_stream();
    loop {
        let next = tokio::select! {
            n = stream.next() => n,
            _ = cancel.cancelled() => return Err(AppError::Cancelled),
        };
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|e| AppError::Download(e.to_string()))?;
        out.write_all(&chunk)?;
        on_bytes(chunk.len() as u64);
    }
    out.flush()?;
    drop(out);
    std::fs::rename(&part, dest)?;
    Ok(())
}

/// Runs a setup command, forwarding its output lines as progress details.
async fn run_step(mut cmd: tokio::process::Command, cancel: &CancellationToken, on_line: &(dyn Fn(String) + Send + Sync)) -> AppResult<()> {
    use tokio::io::AsyncBufReadExt;
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| AppError::Tts(format!("could not start the SILMA installer: {e}")))?;
    let (out, err) = (child.stdout.take().expect("piped"), child.stderr.take().expect("piped"));
    let tail = Arc::new(Mutex::new(Vec::<String>::new()));
    let pump = |r: Box<dyn tokio::io::AsyncRead + Unpin + Send>, tail: Arc<Mutex<Vec<String>>>| async move {
        let mut lines = tokio::io::BufReader::new(r).lines();
        let mut out = Vec::new();
        while let Ok(Some(l)) = lines.next_line().await {
            let l = l.trim().to_string();
            if l.is_empty() {
                continue;
            }
            tracing::debug!(target: "silma_setup", "{l}");
            let mut t = tail.lock().unwrap_or_else(|p| p.into_inner());
            t.push(l.clone());
            if t.len() > 12 {
                t.remove(0);
            }
            out.push(l);
        }
        out
    };
    let (a, b) = (tokio::spawn(pump(Box::new(out), tail.clone())), tokio::spawn(pump(Box::new(err), tail.clone())));
    let tail_watch = tail.clone();
    let status = loop {
        tokio::select! {
            s = child.wait() => break s?,
            _ = cancel.cancelled() => {
                let _ = child.kill().await;
                return Err(AppError::Cancelled);
            }
            _ = tokio::time::sleep(Duration::from_millis(400)) => {
                if let Some(l) = tail_watch.lock().unwrap_or_else(|p| p.into_inner()).last().cloned() {
                    on_line(l);
                }
            }
        }
    };
    let _ = (a.await, b.await);
    if !status.success() {
        let tail = tail.lock().unwrap_or_else(|p| p.into_inner()).join("\n");
        return Err(AppError::Tts(format!("SILMA setup step failed ({status}):\n{tail}")));
    }
    Ok(())
}

/// Sets up SILMA: Python, PyTorch (CUDA build on NVIDIA machines), the SILMA
/// package and its weights, then a test sentence. Resumable: finished steps
/// are skipped on the next attempt.
pub async fn install(data_dir: &Path, nvidia: bool, cancel: CancellationToken, on_progress: &(dyn Fn(SilmaProgress) + Send + Sync)) -> AppResult<()> {
    let d = dir(data_dir);
    let total = if nvidia { GPU_DOWNLOAD_BYTES } else { CPU_DOWNLOAD_BYTES };
    let done = Arc::new(AtomicU64::new(0));
    let report = {
        let done = done.clone();
        move |stage: &str, detail: Option<String>| {
            on_progress(SilmaProgress { stage: stage.into(), downloaded_bytes: done.load(Ordering::Relaxed).min(total), total_bytes: total, detail, error: None })
        }
    };
    let result: AppResult<()> = async {
        std::fs::create_dir_all(d.join("weights"))?;
        let _ = std::fs::remove_file(d.join(".complete"));
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .user_agent(concat!("LocalAssistant/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| AppError::Download(e.to_string()))?;

        // 1. uv: installs Python and packages without a system Python.
        let uv = d.join("uv.exe");
        if !uv.exists() {
            report("runtime", Some("Downloading the Python installer…".into()));
            let zip = d.join("uv.zip");
            download(&client, UV_URL, &zip, &cancel, |n| {
                done.fetch_add(n, Ordering::Relaxed);
            })
            .await?;
            let (zip2, uv2) = (zip.clone(), uv.clone());
            tokio::task::spawn_blocking(move || -> AppResult<()> {
                let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip2)?).map_err(|e| AppError::Download(e.to_string()))?;
                let mut entry = archive.by_name("uv.exe").map_err(|e| AppError::Download(e.to_string()))?;
                let mut out = std::fs::File::create(&uv2)?;
                std::io::copy(&mut entry, &mut out)?;
                Ok(())
            })
            .await
            .map_err(|e| AppError::Download(e.to_string()))??;
            let _ = std::fs::remove_file(&zip);
        } else {
            done.fetch_add(18_000_000, Ordering::Relaxed);
        }
        let uv_cmd = |args: &[&str]| {
            let mut c = tokio::process::Command::new(&uv);
            c.args(args)
                .current_dir(&d)
                .env("UV_PYTHON_INSTALL_DIR", d.join("python"))
                .env("UV_CACHE_DIR", d.join("uv-cache"))
                // Never borrow a Python installed on the machine: SILMA must
                // keep working if the user removes or upgrades it.
                .env("UV_PYTHON_PREFERENCE", "only-managed")
                .env("UV_NO_PROGRESS", "1");
            c
        };
        let line = |stage: &'static str| {
            let report = report.clone();
            move |l: String| report(stage, Some(l))
        };

        // 2. Python + packages. Byte counts are unknown here; the bar moves by step.
        if !python(&d).exists() {
            report("runtime", Some("Installing Python…".into()));
            run_step(uv_cmd(&["venv", "venv", "--python", "3.11", "--seed"]), &cancel, &line("runtime")).await?;
        }
        done.store(48_000_000, Ordering::Relaxed);
        report("packages", Some("Installing PyTorch and SILMA (this is the big part)…".into()));
        let py = python(&d).to_string_lossy().to_string();
        let (torch, torchaudio, index) = if nvidia {
            ("torch==2.8.0+cu128", "torchaudio==2.8.0+cu128", "https://download.pytorch.org/whl/cu128")
        } else {
            ("torch==2.8.0+cpu", "torchaudio==2.8.0+cpu", "https://download.pytorch.org/whl/cpu")
        };
        let mut args = vec!["pip", "install", "--python", &py, "--index-strategy", "unsafe-best-match", "--extra-index-url", index, torch, torchaudio];
        args.extend_from_slice(PACKAGES);
        run_step(uv_cmd(&args), &cancel, &line("packages")).await?;
        run_step(uv_cmd(&["pip", "install", "--python", &py, "--no-deps", SILMA_PACKAGE]), &cancel, &line("packages")).await?;
        trim_torch(&d);
        done.store(total - MODEL_BYTES, Ordering::Relaxed);

        // 3. Weights (skipped once converted).
        let weights = d.join("weights");
        if !weights.join("silma-v1.fp16.safetensors").exists() {
            if !weights.join("model.pt").exists() {
                report("weights", None);
                let mut last = Instant::now();
                download(&client, MODEL_URL, &weights.join("model.pt"), &cancel, |n| {
                    done.fetch_add(n, Ordering::Relaxed);
                    if last.elapsed() > Duration::from_millis(250) {
                        report("weights", None);
                        last = Instant::now();
                    }
                })
                .await?;
            }
        }
        if !weights.join("vocab.txt").exists() {
            download(&client, VOCAB_URL, &weights.join("vocab.txt"), &cancel, |_| {}).await?;
        }
        done.store(total, Ordering::Relaxed);

        // 4. Slim the checkpoint, fetch vocoder + tashkeel models, test sentence.
        report("prepare", Some("Preparing the voice (first run downloads two small models)…".into()));
        std::fs::write(d.join("server.py"), SERVER_PY)?;
        let mut prep = tokio::process::Command::new(python(&d));
        prep.arg(d.join("server.py"))
            .arg("--prepare")
            .current_dir(&d)
            .env("HF_HOME", d.join("hf"))
            .env("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
            .env("PYTHONIOENCODING", "utf-8");
        run_step(prep, &cancel, &line("prepare")).await?;

        let _ = std::fs::remove_file(weights.join("model.pt"));
        let _ = std::fs::remove_dir_all(d.join("uv-cache"));
        std::fs::write(d.join(".complete"), b"1")?;
        Ok(())
    }
    .await;

    match &result {
        Ok(()) => {
            tracing::info!(dir = %d.display(), "SILMA installed");
            on_progress(SilmaProgress { stage: "done".into(), downloaded_bytes: total, total_bytes: total, detail: None, error: None });
        }
        Err(AppError::Cancelled) => {
            on_progress(SilmaProgress { stage: "cancelled".into(), downloaded_bytes: 0, total_bytes: total, detail: None, error: None });
        }
        Err(e) => {
            tracing::warn!(error = %e, "SILMA setup failed");
            on_progress(SilmaProgress { stage: "error".into(), downloaded_bytes: 0, total_bytes: total, detail: None, error: Some(e.to_string()) });
        }
    }
    result
}

/// PyTorch ships ~2.7 GB of C++ link libraries and headers that only matter
/// for compiling extensions; running never touches them.
fn trim_torch(d: &Path) {
    let torch = d.join("venv").join("Lib").join("site-packages").join("torch");
    let _ = std::fs::remove_dir_all(torch.join("include"));
    if let Ok(entries) = std::fs::read_dir(torch.join("lib")) {
        for e in entries.flatten() {
            if e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("lib")) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

// ---------------------------------------------------------------- runtime

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reply {
    id: Option<u64>,
    event: Option<String>,
    ok: Option<bool>,
    device: Option<String>,
    error: Option<String>,
    sample_rate: Option<u32>,
    pcm: Option<String>,
}

struct Proc {
    child: Child,
    stdin: ChildStdin,
}

#[derive(Default)]
struct RunState {
    state: String,
    device: Option<String>,
    error: Option<String>,
}

pub struct Silma {
    data_dir: PathBuf,
    nvidia: bool,
    proc: Mutex<Option<Proc>>,
    run: Arc<(Mutex<RunState>, Condvar)>,
    pending: Arc<Mutex<HashMap<u64, Sender<Reply>>>>,
    next_id: AtomicU64,
    emit: Arc<dyn Fn(SilmaStatus) + Send + Sync>,
}

impl Silma {
    pub fn new(data_dir: PathBuf, nvidia: bool, emit: Arc<dyn Fn(SilmaStatus) + Send + Sync>) -> Self {
        Self {
            data_dir,
            nvidia,
            proc: Mutex::new(None),
            run: Arc::new((Mutex::new(RunState { state: "off".into(), ..Default::default() }), Condvar::new())),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            emit,
        }
    }

    pub fn is_installed(&self) -> bool {
        is_installed(&self.data_dir)
    }

    pub fn status(&self) -> SilmaStatus {
        let run = self.run.0.lock().unwrap_or_else(|p| p.into_inner());
        let installed = self.is_installed();
        SilmaStatus {
            installed,
            state: run.state.clone(),
            device: run.device.clone(),
            error: run.error.clone(),
            size_bytes: if installed { dir_size(&dir(&self.data_dir)) } else { 0 },
            download_bytes: if self.nvidia { GPU_DOWNLOAD_BYTES } else { CPU_DOWNLOAD_BYTES },
            gpu: self.nvidia,
        }
    }

    fn set_state(&self, state: &str, device: Option<String>, error: Option<String>) {
        set_run(&self.run, state, device, error);
        (self.emit)(self.status());
    }

    /// Starts the helper in the background (idempotent). The model loads for a
    /// few seconds; [`Self::synthesize`] waits for it.
    pub fn start(&self) -> AppResult<()> {
        if !self.is_installed() {
            return Err(AppError::Tts("SILMA is not installed".into()));
        }
        let mut guard = self.proc.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(p) = guard.as_mut() {
            if matches!(p.child.try_wait(), Ok(None)) {
                return Ok(());
            }
            *guard = None;
        }
        let d = dir(&self.data_dir);
        // Always the server of this app version.
        std::fs::write(d.join("server.py"), SERVER_PY)?;
        let mut cmd = Command::new(python(&d));
        cmd.arg(d.join("server.py"))
            .current_dir(&d)
            .env("HF_HOME", d.join("hf"))
            .env("HF_HUB_OFFLINE", "1")
            .env("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = no_window(&mut cmd).spawn().map_err(|e| AppError::Tts(format!("could not start SILMA: {e}")))?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        self.set_state("starting", None, None);
        tracing::info!("SILMA helper started");

        std::thread::Builder::new()
            .name("silma-log".into())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        tracing::debug!(target: "silma", "{line}");
                    }
                }
            })
            .map_err(|e| AppError::Tts(e.to_string()))?;

        let (run, pending, emit) = (self.run.clone(), self.pending.clone(), self.emit.clone());
        let data_dir = self.data_dir.clone();
        let nvidia = self.nvidia;
        std::thread::Builder::new()
            .name("silma-reader".into())
            .spawn(move || {
                let status = |run: &Arc<(Mutex<RunState>, Condvar)>| {
                    let r = run.0.lock().unwrap_or_else(|p| p.into_inner());
                    SilmaStatus {
                        installed: is_installed(&data_dir),
                        state: r.state.clone(),
                        device: r.device.clone(),
                        error: r.error.clone(),
                        size_bytes: 0,
                        download_bytes: if nvidia { GPU_DOWNLOAD_BYTES } else { CPU_DOWNLOAD_BYTES },
                        gpu: nvidia,
                    }
                };
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let Ok(reply) = serde_json::from_str::<Reply>(&line) else { continue };
                    match reply.event.as_deref() {
                        Some("ready") => {
                            tracing::info!(device = ?reply.device, "SILMA ready");
                            set_run(&run, "ready", reply.device.clone(), None);
                            emit(status(&run));
                        }
                        Some("failed") => {
                            tracing::warn!(error = ?reply.error, "SILMA failed to load");
                            set_run(&run, "failed", None, reply.error.clone());
                            emit(status(&run));
                        }
                        _ => {
                            if let Some(tx) = reply.id.and_then(|id| pending.lock().unwrap_or_else(|p| p.into_inner()).remove(&id)) {
                                let _ = tx.send(reply);
                            }
                        }
                    }
                }
                // Process ended: fail whatever is still waiting.
                pending.lock().unwrap_or_else(|p| p.into_inner()).clear();
                let was_failed = run.0.lock().unwrap_or_else(|p| p.into_inner()).state == "failed";
                if !was_failed {
                    set_run(&run, "off", None, None);
                }
                emit(status(&run));
                tracing::info!("SILMA helper exited");
            })
            .map_err(|e| AppError::Tts(e.to_string()))?;

        *guard = Some(Proc { child, stdin });
        Ok(())
    }

    /// Speaks `text` (Arabic and/or English) at 24 kHz. Blocking.
    pub fn synthesize(&self, text: &str, speed: f32) -> AppResult<(Vec<f32>, u32)> {
        self.start()?;
        {
            let (lock, cv) = &*self.run;
            let guard = lock.lock().unwrap_or_else(|p| p.into_inner());
            let (guard, timeout) = cv
                .wait_timeout_while(guard, LOAD_TIMEOUT, |r| r.state == "starting")
                .unwrap_or_else(|p| p.into_inner());
            if timeout.timed_out() {
                return Err(AppError::Tts("SILMA took too long to load".into()));
            }
            if guard.state != "ready" {
                return Err(AppError::Tts(guard.error.clone().unwrap_or_else(|| "SILMA is not running".into())));
            }
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap_or_else(|p| p.into_inner()).insert(id, tx);
        let req = serde_json::json!({ "id": id, "cmd": "say", "text": text, "speed": speed });
        {
            let mut guard = self.proc.lock().unwrap_or_else(|p| p.into_inner());
            let p = guard.as_mut().ok_or_else(|| AppError::Tts("SILMA is not running".into()))?;
            writeln!(p.stdin, "{req}").and_then(|_| p.stdin.flush()).map_err(|e| AppError::Tts(format!("SILMA stopped: {e}")))?;
        }
        let reply = rx.recv_timeout(SAY_TIMEOUT).map_err(|_| {
            self.pending.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
            AppError::Tts("SILMA did not answer in time".into())
        })?;
        if reply.ok != Some(true) {
            return Err(AppError::Tts(reply.error.unwrap_or_else(|| "SILMA synthesis failed".into())));
        }
        let bytes = crate::services::attachments::base64::decode(reply.pcm.as_deref().unwrap_or_default()).map_err(AppError::Tts)?;
        let samples = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        Ok((samples, reply.sample_rate.unwrap_or(24_000)))
    }

    /// Stops the helper (frees its GPU memory).
    pub fn stop(&self) {
        if let Some(mut p) = self.proc.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = writeln!(p.stdin, "{{\"cmd\":\"quit\"}}");
            drop(p.stdin);
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(2) {
                if matches!(p.child.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = p.child.kill();
        }
    }

    pub fn remove(&self) -> AppResult<()> {
        self.stop();
        let d = dir(&self.data_dir);
        if d.exists() {
            std::fs::remove_dir_all(&d)?;
        }
        self.set_state("off", None, None);
        Ok(())
    }
}

fn set_run(run: &Arc<(Mutex<RunState>, Condvar)>, state: &str, device: Option<String>, error: Option<String>) {
    let (lock, cv) = &**run;
    let mut r = lock.lock().unwrap_or_else(|p| p.into_inner());
    r.state = state.into();
    r.device = device;
    r.error = error;
    cv.notify_all();
}

impl Drop for Silma {
    fn drop(&mut self) {
        self.stop();
    }
}
