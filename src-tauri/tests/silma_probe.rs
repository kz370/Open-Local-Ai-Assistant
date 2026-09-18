//! Manual probe: the app's SILMA client starts the helper, gets Arabic speech
//! back and stops it. Needs SILMA set up in the app data folder. Run with:
//! `cargo test --test silma_probe -- --ignored --nocapture`

use local_ai_assistant_lib::services::gpu::default_data_dir;
use local_ai_assistant_lib::services::silma::Silma;
use std::sync::Arc;
use std::time::Instant;

#[test]
#[ignore]
fn silma_speaks_arabic() {
    let data_dir = default_data_dir().expect("APPDATA");
    let silma = Silma::new(data_dir, true, Arc::new(|s| eprintln!("status: {} {:?} {:?}", s.state, s.device, s.error)));
    assert!(silma.is_installed(), "SILMA is not set up");
    let t0 = Instant::now();
    let (samples, rate) = silma.synthesize("كيف يمكنني مساعدتك اليوم؟ لديك 3 رسائل جديدة.", 1.0).expect("synthesize");
    let secs = samples.len() as f32 / rate as f32;
    eprintln!("first sentence (incl. load): {secs:.1}s audio in {:.1}s", t0.elapsed().as_secs_f32());
    assert_eq!(rate, 24_000);
    assert!(secs > 1.0, "too little audio");
    assert!(samples.iter().any(|s| s.abs() > 0.05), "silent audio");
    let t1 = Instant::now();
    let (samples, rate) = silma.synthesize("شكرا لك.", 1.0).expect("synthesize");
    eprintln!("second sentence: {:.1}s audio in {:.1}s", samples.len() as f32 / rate as f32, t1.elapsed().as_secs_f32());
    silma.stop();
}

/// Re-runs the in-app setup over an existing install: every step runs, but
/// the big downloads are skipped because their results are already there.
#[tokio::test]
#[ignore]
async fn setup_is_resumable() {
    let data_dir = default_data_dir().expect("APPDATA");
    let stages = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let s2 = stages.clone();
    let progress = move |p: local_ai_assistant_lib::services::silma::SilmaProgress| {
        let mut s = s2.lock().unwrap();
        if s.last() != Some(&p.stage) {
            eprintln!("stage {} {:?} {:?}", p.stage, p.detail, p.error);
            s.push(p.stage.clone());
        }
    };
    let t0 = Instant::now();
    local_ai_assistant_lib::services::silma::install(&data_dir, true, tokio_util::sync::CancellationToken::new(), &progress)
        .await
        .expect("setup");
    eprintln!("setup took {:.0}s", t0.elapsed().as_secs_f32());
    assert_eq!(stages.lock().unwrap().last().map(String::as_str), Some("done"));
    assert!(local_ai_assistant_lib::services::silma::is_installed(&data_dir));
}
