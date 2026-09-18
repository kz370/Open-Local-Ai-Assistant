//! Manual probe: does WASAPI loopback level tracking actually see speaker audio?
//! Run with: cargo test --test loopback_probe -- --ignored --nocapture

use local_ai_assistant_lib::services::audio::capture::LoopbackMonitor;
use local_ai_assistant_lib::services::audio::playback::{Clip, Player};

#[test]
#[ignore]
fn loopback_sees_playback() {
    let monitor = match LoopbackMonitor::start(None) {
        Ok(m) => m,
        Err(e) => panic!("loopback monitor did not start: {e}"),
    };
    let player = Player::new(None);
    std::thread::sleep(std::time::Duration::from_millis(800));
    assert!(player.is_available(), "no output device");
    let sr = 22_050u32;
    let samples: Vec<f32> = (0..sr * 2).map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / sr as f32).sin() * 0.3).collect();
    player.enqueue(Clip { samples, sample_rate: sr, tag: "probe".into() }).expect("enqueue");
    let mut max_playing = 0.0f32;
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        max_playing = max_playing.max(monitor.level());
    }
    player.stop(None);
    std::thread::sleep(std::time::Duration::from_millis(400));
    let after = monitor.level();
    println!("max level while playing = {max_playing:.3}, level after stop = {after:.3}");
    assert!(max_playing > 0.12, "loopback level never crossed the gate threshold");
}
