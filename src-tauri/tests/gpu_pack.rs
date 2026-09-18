//! Installs the CUDA GPU pack into a chosen folder and checks it is complete.
//! Run with: LA_GPU_DIR=<app data dir> cargo test --test gpu_pack -- --ignored --nocapture

use local_ai_assistant_lib::services::gpu;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore]
async fn installs_the_cuda_pack() {
    let dir: PathBuf = std::env::var("LA_GPU_DIR").expect("set LA_GPU_DIR").into();
    let progress = |p: gpu::GpuProgress| {
        if p.state != "downloading" || p.downloaded_bytes % (50 * 1024 * 1024) < 2_000_000 {
            println!("{} {} / {}", p.state, p.downloaded_bytes, p.total_bytes);
        }
    };
    gpu::install(&dir, CancellationToken::new(), &progress).await.expect("install failed");
    assert!(gpu::is_installed(&dir), "pack incomplete after install");
    let pack = gpu::pack_dir(&dir);
    let mut dlls: Vec<String> = std::fs::read_dir(&pack)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".dll"))
        .collect();
    dlls.sort();
    println!("{} libraries in {}", dlls.len(), pack.display());
    for d in &dlls {
        println!("  {d}");
    }
}
