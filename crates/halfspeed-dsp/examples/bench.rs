//! Rough CPU benchmark: `cargo run --release -p halfspeed-dsp --example bench`.
//! Reports real-time factor and the worst single-block time per host block size, then the
//! worst time to reset at each sample rate (hosts may reset on the audio thread).

use std::time::{Duration, Instant};

use halfspeed_dsp::{HalfSpeedEngine, Settings, Timing};

fn main() {
    let sample_rate = 48_000.0;
    let seconds = 20.0;
    let tempo = 128.0;
    let settings = Settings {
        ratio: 0.5,
        loop_beats: 1.0,
        smoothness: 0.5,
        // Both band filters engaged: the most expensive configuration.
        low_cut_hz: 180.0,
        high_cut_hz: 6500.0,
        ..Settings::default()
    };

    for block_size in [32, 64, 128, 256] {
        let mut engine = HalfSpeedEngine::new(sample_rate, 2, 20.0);
        let mut left: Vec<f32> = (0..block_size).map(|i| (i as f32 * 0.05).sin()).collect();
        let mut right = left.clone();
        let blocks = (sample_rate * seconds) as usize / block_size;
        let block_duration = Duration::from_secs_f64(block_size as f64 / sample_rate as f64);

        let mut worst = Duration::ZERO;
        let start = Instant::now();
        for block in 0..blocks {
            let beat = (block * block_size) as f64 / sample_rate as f64 * tempo / 60.0;
            let timing = Timing {
                tempo_bpm: tempo,
                beat: Some(beat),
            };
            let t = Instant::now();
            engine.process(&mut [&mut left, &mut right], &settings, &timing);
            worst = worst.max(t.elapsed());
        }
        let total = start.elapsed();

        println!(
            "block {block_size:>3}: {:>6.1}x real time, worst block {:>6.1} us of {:>6.1} us budget ({:.1}%)",
            seconds as f64 / total.as_secs_f64(),
            worst.as_secs_f64() * 1e6,
            block_duration.as_secs_f64() * 1e6,
            worst.as_secs_f64() / block_duration.as_secs_f64() * 100.0,
        );
    }

    for sample_rate in [48_000.0, 96_000.0, 192_000.0] {
        let mut engine = HalfSpeedEngine::new(sample_rate, 2, 40.0);
        let mut worst = Duration::ZERO;
        for _ in 0..10 {
            let t = Instant::now();
            engine.reset();
            worst = worst.max(t.elapsed());
        }
        println!(
            "reset at {:>3} kHz: worst {:>7.3} us",
            sample_rate / 1000.0,
            worst.as_secs_f64() * 1e6
        );
    }
}
