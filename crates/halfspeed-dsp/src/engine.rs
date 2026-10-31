//! The half-speed loop engine.
//!
//! Each loop cycle replays the audio that arrived since the cycle started, slowed down by the
//! playback ratio. At ratio `r`, the sample played `t` samples into a cycle was captured at
//! `r * t`, which is always in the past, so the effect needs no lookahead and adds no latency.
//! Over a cycle of length `L` the engine plays the first `r * L` samples of the cycle stretched
//! to fill all of it, and the read head lags the input by up to `(1 - r) * L` samples.
//!
//! When a cycle restarts, the read head jumps back to the present. To hide that jump, the old
//! head keeps playing and is crossfaded into the new one over a length set by `smoothness`.

use std::f32::consts::FRAC_PI_2;

use crate::band::{self, Band};
use crate::clock::LoopClock;
use crate::ring_buffer::RingBuffer;

/// The shortest crossfade, used at 0% smoothness, to avoid clicks at cycle boundaries.
const MIN_CROSSFADE_SECONDS: f64 = 0.001;
/// At 100% smoothness, the crossfade lasts this fraction of the loop.
const MAX_CROSSFADE_LOOP_FRACTION: f64 = 0.25;
/// Mix changes are smoothed over this time to avoid zipper noise.
const MIX_SMOOTHING_SECONDS: f32 = 0.02;
/// Even with fades off, switching the effect is ramped over this time to avoid a click.
const MIN_FADE_SECONDS: f32 = 0.005;
/// Number of slots in the per-cycle waveform overview.
pub const SCOPE_BINS: usize = 256;
/// Used when the host doesn't report a tempo.
pub const DEFAULT_TEMPO_BPM: f64 = 120.0;

/// Per-block settings, normally taken from plugin parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    /// Playback speed of the loop relative to the input, in `(0, 1]`.
    pub ratio: f64,
    /// Loop length in quarter-note beats.
    pub loop_beats: f64,
    /// `0..=1`: lengthens the crossfade at each loop restart.
    pub smoothness: f32,
    /// Time to fade the effect in when `enabled` turns on.
    pub fade_in_seconds: f32,
    /// Time to fade the effect out when `enabled` turns off.
    pub fade_out_seconds: f32,
    /// High-pass cutoff for the wet signal; at [`band::MIN_HZ`] the filter is off.
    pub low_cut_hz: f32,
    /// Low-pass cutoff for the wet signal; at [`band::MAX_HZ`] the filter is off.
    pub high_cut_hz: f32,
    /// `0..=1` dry/wet balance.
    pub mix: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            ratio: 0.5,
            loop_beats: 4.0,
            smoothness: 0.3,
            fade_in_seconds: 0.0,
            fade_out_seconds: 0.0,
            low_cut_hz: band::MIN_HZ,
            high_cut_hz: band::MAX_HZ,
            mix: 1.0,
        }
    }
}

/// Host timing for the first sample of a block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    pub tempo_bpm: f64,
    /// Position in quarter notes, or `None` to free-run (transport stopped or sync off).
    pub beat: Option<f64>,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            tempo_bpm: DEFAULT_TEMPO_BPM,
            beat: None,
        }
    }
}

/// A linear ramp towards a target value.
#[derive(Debug, Clone, Copy, Default)]
struct Ramp {
    value: f32,
    target: f32,
    step: f32,
}

impl Ramp {
    fn jump_to(&mut self, value: f32) {
        *self = Self {
            value,
            target: value,
            step: 0.0,
        };
    }

    /// Ramps to `target` over `samples` samples. Calling it again with the same target keeps the
    /// ramp going instead of restarting it.
    fn set_target(&mut self, target: f32, samples: f32) {
        if target != self.target {
            self.target = target;
            self.step = (target - self.value).abs() / samples.max(1.0);
        }
    }

    fn next(&mut self) -> f32 {
        if self.value < self.target {
            self.value = (self.value + self.step).min(self.target);
        } else if self.value > self.target {
            self.value = (self.value - self.step).max(self.target);
        }
        self.value
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Crossfade {
    /// Age of the outgoing read head.
    old_age: f64,
    pos: f64,
    len: f64,
}

pub struct HalfSpeedEngine {
    sample_rate: f64,
    buffer: RingBuffer,
    clock: LoopClock,
    /// Age of the read head, in samples behind the newest input frame.
    age: f64,
    crossfade: Option<Crossfade>,
    /// Whether a cycle has played since the last reset, i.e. whether there is an old head to
    /// crossfade from. Crossfading a head with itself would boost the level by up to 3 dB.
    has_cycle: bool,
    /// Gain of the wet path from the enabled state, faded by the fade in/out times.
    effect_gain: Ramp,
    mix: Ramp,
    /// Whether the ramps have been snapped to their first targets since the last reset.
    primed: bool,
    /// One frame of input, preallocated so `process` doesn't allocate.
    frame: Vec<f32>,
    loop_samples: f64,
    band: Band,
    /// Peak level of the wet signal across the current cycle, for display.
    scope: [f32; SCOPE_BINS],
    scope_bin: usize,
}

impl HalfSpeedEngine {
    /// Allocates for up to `max_loop_seconds` of lag. Not real-time safe.
    pub fn new(sample_rate: f32, num_channels: usize, max_loop_seconds: f32) -> Self {
        let mut engine = Self {
            sample_rate: 0.0,
            buffer: RingBuffer::new(0, 1),
            clock: LoopClock::default(),
            age: 0.0,
            crossfade: None,
            has_cycle: false,
            effect_gain: Ramp::default(),
            mix: Ramp::default(),
            primed: false,
            frame: Vec::new(),
            loop_samples: 1.0,
            band: Band::default(),
            scope: [0.0; SCOPE_BINS],
            scope_bin: 0,
        };
        engine.prepare(sample_rate, num_channels, max_loop_seconds);
        engine
    }

    /// Sizes buffers for a sample rate and channel layout, then resets. Reallocates only when
    /// the layout changed, as hosts may call this repeatedly while restoring state. Not
    /// real-time safe.
    pub fn prepare(&mut self, sample_rate: f32, num_channels: usize, max_loop_seconds: f32) {
        let capacity = ((sample_rate * max_loop_seconds).ceil() as usize).max(1);
        if self.sample_rate != sample_rate as f64
            || self.buffer.num_channels() != num_channels
            || self.buffer.capacity() != capacity
        {
            self.sample_rate = sample_rate as f64;
            self.buffer.resize(num_channels, capacity);
            self.frame.clear();
            self.frame.resize(num_channels, 0.0);
            self.band.prepare(sample_rate, num_channels);
        }
        self.reset();
    }

    /// Clears captured audio and restarts the loop, e.g. when the host resets the plugin.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.clock.reset();
        self.age = 0.0;
        self.crossfade = None;
        self.has_cycle = false;
        self.primed = false;
        self.band.reset();
        self.scope = [0.0; SCOPE_BINS];
        self.scope_bin = 0;
    }

    /// Progress through the current loop cycle in `0..1`, for display.
    pub fn cycle_phase(&self) -> f32 {
        (self.clock.position() / self.loop_samples).clamp(0.0, 1.0) as f32
    }

    /// Peak wet level per slice of the loop cycle, for display. Slots ahead of the current
    /// position still hold the previous cycle.
    pub fn scope(&self) -> &[f32; SCOPE_BINS] {
        &self.scope
    }

    /// Processes one block in place. `channels` holds one slice per channel, all the same length.
    pub fn process(&mut self, channels: &mut [&mut [f32]], settings: &Settings, timing: &Timing) {
        let num_frames = channels.first().map_or(0, |c| c.len());
        let num_channels = channels.len().min(self.frame.len());

        let sample_rate = self.sample_rate as f32;
        let effect_target = if settings.enabled { 1.0 } else { 0.0 };
        let mix_target = settings.mix.clamp(0.0, 1.0);
        if !self.primed {
            self.effect_gain.jump_to(effect_target);
            self.mix.jump_to(mix_target);
            self.primed = true;
        }
        let fade_seconds = if settings.enabled {
            settings.fade_in_seconds
        } else {
            settings.fade_out_seconds
        };
        self.effect_gain.set_target(
            effect_target,
            fade_seconds.max(MIN_FADE_SECONDS) * sample_rate,
        );
        self.mix
            .set_target(mix_target, MIX_SMOOTHING_SECONDS * sample_rate);

        let tempo = if timing.tempo_bpm > 0.0 {
            timing.tempo_bpm
        } else {
            DEFAULT_TEMPO_BPM
        };
        let samples_per_beat = self.sample_rate * 60.0 / tempo;
        let ratio = settings.ratio.clamp(0.01, 1.0);
        self.loop_samples = (settings.loop_beats * samples_per_beat).max(1.0);
        let crossfade_len = self.crossfade_len(settings.smoothness);
        // Ages beyond this would read past the oldest captured frame.
        let max_age = self.buffer.capacity().saturating_sub(2) as f64;

        for i in 0..num_frames {
            for (channel, sample) in self.frame.iter_mut().enumerate().take(num_channels) {
                *sample = channels[channel][i];
            }
            self.buffer.push_frame(&self.frame);

            let beat = timing.beat.map(|b| b + i as f64 / samples_per_beat);
            if let Some(offset) = self.clock.tick(self.loop_samples, samples_per_beat, beat) {
                self.crossfade = (self.has_cycle && crossfade_len > 0.0).then_some(Crossfade {
                    old_age: self.age,
                    pos: 0.0,
                    len: crossfade_len,
                });
                self.age = offset * (1.0 - ratio);
                self.has_cycle = true;
            }
            self.age = self.age.min(max_age);

            // Equal-power gains, as the two heads usually play unrelated material.
            let (old_gain, new_gain) = match &self.crossfade {
                Some(fade) => {
                    let t = (fade.pos / fade.len) as f32 * FRAC_PI_2;
                    (t.cos(), t.sin())
                }
                None => (0.0, 1.0),
            };
            let wet_amount = self.effect_gain.next() * self.mix.next();
            self.band.tick(settings.low_cut_hz, settings.high_cut_hz);

            let mut peak = 0.0f32;
            for (channel, samples) in channels.iter_mut().enumerate().take(num_channels) {
                let dry = self.frame[channel];
                let mut wet = new_gain * self.buffer.get_interpolated(channel, self.age);
                if let Some(fade) = &self.crossfade {
                    wet += old_gain * self.buffer.get_interpolated(channel, fade.old_age);
                }
                let wet = self.band.process(channel, wet);
                peak = peak.max(wet.abs());
                samples[i] = dry + (wet - dry) * wet_amount;
            }
            self.record_scope(peak);

            // Each new input frame ages the heads by one sample while they advance by `ratio`.
            self.age += 1.0 - ratio;
            if let Some(fade) = &mut self.crossfade {
                fade.old_age = (fade.old_age + 1.0 - ratio).min(max_age);
                fade.pos += 1.0;
                if fade.pos >= fade.len {
                    self.crossfade = None;
                }
            }
        }
    }

    fn record_scope(&mut self, peak: f32) {
        let phase = self.clock.position() / self.loop_samples;
        let bin = ((phase * SCOPE_BINS as f64) as usize).min(SCOPE_BINS - 1);
        if bin != self.scope_bin {
            self.scope_bin = bin;
            self.scope[bin] = 0.0;
        }
        self.scope[bin] = self.scope[bin].max(peak);
    }

    fn crossfade_len(&self, smoothness: f32) -> f64 {
        let min = MIN_CROSSFADE_SECONDS * self.sample_rate;
        let max = (self.loop_samples * MAX_CROSSFADE_LOOP_FRACTION).max(min);
        let len = min + (max - min) * smoothness.clamp(0.0, 1.0) as f64;
        len.min(self.loop_samples / 2.0).floor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1 kHz with 60 BPM gives 1000 samples per beat, which keeps expected positions readable.
    const SAMPLE_RATE: f32 = 1000.0;
    const TEMPO: f64 = 60.0;

    fn ramp(len: usize) -> Vec<f32> {
        (0..len).map(|i| i as f32).collect()
    }

    fn sine(len: usize, sample_rate: f32, freq: f32) -> Vec<f32> {
        (0..len)
            .map(|i| (i as f32 / sample_rate * freq * std::f32::consts::TAU).sin())
            .collect()
    }

    /// Runs a mono signal through the engine in blocks, passing host-style timing per block.
    fn run(
        engine: &mut HalfSpeedEngine,
        input: &[f32],
        block_size: usize,
        settings: &Settings,
        synced: bool,
    ) -> Vec<f32> {
        let samples_per_beat = SAMPLE_RATE as f64 * 60.0 / TEMPO;
        let mut output = input.to_vec();
        let mut start = 0;
        for block in output.chunks_mut(block_size) {
            let timing = Timing {
                tempo_bpm: TEMPO,
                beat: synced.then(|| start as f64 / samples_per_beat),
            };
            start += block.len();
            engine.process(&mut [block], settings, &timing);
        }
        output
    }

    fn settings(ratio: f64, loop_beats: f64) -> Settings {
        Settings {
            ratio,
            loop_beats,
            smoothness: 0.0,
            ..Settings::default()
        }
    }

    #[test]
    fn follows_the_input_at_the_playback_ratio() {
        for ratio in [0.5, 2.0 / 3.0, 0.25] {
            let mut engine = HalfSpeedEngine::new(SAMPLE_RATE, 1, 10.0);
            let output = run(&mut engine, &ramp(3000), 64, &settings(ratio, 1.0), true);

            // At 0% smoothness the crossfade is 1 ms = 1 sample, so from the second sample of
            // each 1000-sample cycle the output must be exactly the slowed-down input.
            for (n, &y) in output.iter().enumerate() {
                let cycle_start = n / 1000 * 1000;
                let t = n - cycle_start;
                if t < 1 {
                    continue;
                }
                let expected = cycle_start as f64 + ratio * t as f64;
                assert!(
                    (y as f64 - expected).abs() < 1e-3,
                    "ratio {ratio}, sample {n}: got {y}, expected {expected}"
                );
            }
        }
    }

    #[test]
    fn output_does_not_depend_on_block_size() {
        let input = sine(5000, SAMPLE_RATE, 7.0);
        let settings = Settings {
            smoothness: 0.6,
            ..settings(0.5, 0.75)
        };
        for synced in [true, false] {
            let reference = run(
                &mut HalfSpeedEngine::new(SAMPLE_RATE, 1, 10.0),
                &input,
                1,
                &settings,
                synced,
            );
            for block_size in [32, 64, 128, 256, 333] {
                let output = run(
                    &mut HalfSpeedEngine::new(SAMPLE_RATE, 1, 10.0),
                    &input,
                    block_size,
                    &settings,
                    synced,
                );
                for (n, (a, b)) in reference.iter().zip(&output).enumerate() {
                    assert!(
                        (a - b).abs() < 1e-5,
                        "synced {synced}, block {block_size}, sample {n}: {a} vs {b}"
                    );
                }
            }
        }
    }

    #[test]
    fn zero_mix_and_disabled_pass_input_unchanged() {
        let input = sine(2000, SAMPLE_RATE, 13.0);
        let dry_settings = [
            Settings {
                mix: 0.0,
                ..Settings::default()
            },
            Settings {
                enabled: false,
                ..Settings::default()
            },
        ];
        for settings in dry_settings {
            let output = run(
                &mut HalfSpeedEngine::new(SAMPLE_RATE, 1, 10.0),
                &input,
                64,
                &settings,
                true,
            );
            assert_eq!(output, input, "{settings:?}");
        }
    }

    #[test]
    fn enabling_fades_in_over_the_fade_time() {
        // With a ramp input x[n] = n and ratio 0.5, the first cycle's wet signal is n / 2, so
        // the output n * (1 - g / 2) reveals the effect gain g.
        let mut engine = HalfSpeedEngine::new(SAMPLE_RATE, 1, 10.0);
        let mut input = ramp(400);
        let (off_block, on_block) = input.split_at_mut(10);
        let off = Settings {
            enabled: false,
            ..Settings::default()
        };
        let on = Settings {
            fade_in_seconds: 0.1,
            ..Settings::default()
        };
        engine.process(&mut [off_block], &off, &Timing::default());
        engine.process(&mut [on_block], &on, &Timing::default());

        let gain_at = |n: usize| 2.0 * (1.0 - input[n] / n as f32);
        // 0.1 s at 1 kHz = 100 samples, so the gain rises by 0.01 per sample.
        assert!((gain_at(10) - 0.01).abs() < 1e-3, "{}", gain_at(10));
        assert!((gain_at(59) - 0.5).abs() < 1e-3, "{}", gain_at(59));
        assert!((gain_at(109) - 1.0).abs() < 1e-3, "{}", gain_at(109));
        assert!((gain_at(300) - 1.0).abs() < 1e-3, "{}", gain_at(300));
    }

    #[test]
    fn loop_restarts_are_click_free() {
        let sample_rate = 48_000.0;
        let input = sine(48_000, sample_rate, 440.0);
        let max_input_step = input
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max);

        for smoothness in [0.0, 0.5, 1.0] {
            let mut engine = HalfSpeedEngine::new(sample_rate, 1, 10.0);
            let settings = Settings {
                smoothness,
                loop_beats: 0.25,
                ..Settings::default()
            };
            let mut output = input.clone();
            for block in output.chunks_mut(128) {
                engine.process(
                    &mut [block],
                    &settings,
                    &Timing {
                        tempo_bpm: 120.0,
                        beat: None,
                    },
                );
            }

            assert!(output.iter().all(|s| s.is_finite() && s.abs() <= 1.5));
            let max_step = output
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0, f32::max);
            // A hard cut between unrelated sine phases can jump by up to 2.0; the crossfade
            // must keep restarts in the same range as the signal's own slope.
            assert!(
                max_step < 2.0 * max_input_step,
                "smoothness {smoothness}: max step {max_step} vs input {max_input_step}"
            );
        }
    }

    #[test]
    fn reset_engine_plays_like_a_fresh_one() {
        let settings = Settings {
            smoothness: 0.5,
            ..settings(0.5, 1.0)
        };
        let input = sine(5000, SAMPLE_RATE, 37.0);
        // Starting playback mid-cycle makes the read head reach back past the first sample
        // played, where only silence may be heard, never audio from before the reset.
        let play = |engine: &mut HalfSpeedEngine| {
            let mut output = input.clone();
            for (n, block) in output.chunks_mut(64).enumerate() {
                let timing = Timing {
                    tempo_bpm: TEMPO,
                    beat: Some(0.6 + (n * 64) as f64 / 1000.0),
                };
                engine.process(&mut [block], &settings, &timing);
            }
            output
        };

        let mut fresh = HalfSpeedEngine::new(SAMPLE_RATE, 1, 10.0);
        let expected = play(&mut fresh);

        // Fill the whole buffer with other audio first, so stale samples would show up.
        let mut reused = HalfSpeedEngine::new(SAMPLE_RATE, 1, 10.0);
        run(&mut reused, &ramp(12_000), 64, &settings, false);
        reused.reset();
        assert_eq!(play(&mut reused), expected);
    }

    #[test]
    fn stereo_channels_are_processed_independently() {
        let mut engine = HalfSpeedEngine::new(SAMPLE_RATE, 2, 10.0);
        let mut left = ramp(2000);
        let mut right: Vec<f32> = ramp(2000).iter().map(|x| -x).collect();
        let settings = settings(0.5, 1.0);
        for (l, r) in left.chunks_mut(64).zip(right.chunks_mut(64)) {
            engine.process(
                &mut [l, r],
                &settings,
                &Timing {
                    tempo_bpm: TEMPO,
                    beat: None,
                },
            );
        }
        assert_eq!(left[1500], 1250.0);
        assert_eq!(right[1500], -1250.0);
    }

    #[test]
    fn loops_longer_than_the_buffer_stay_bounded() {
        let mut engine = HalfSpeedEngine::new(SAMPLE_RATE, 1, 0.5);
        let input = sine(5000, SAMPLE_RATE, 3.0);
        let output = run(&mut engine, &input, 64, &settings(0.25, 4.0), true);
        assert!(
            output
                .iter()
                .all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-6)
        );
    }

    #[test]
    fn fading_out_uses_the_fade_out_time() {
        // Same ramp trick as above, switching off with a 50 ms fade-out.
        let mut engine = HalfSpeedEngine::new(SAMPLE_RATE, 1, 10.0);
        let mut input = ramp(200);
        let (on_block, off_block) = input.split_at_mut(10);
        let on = Settings::default();
        let off = Settings {
            enabled: false,
            fade_in_seconds: 1.0,
            fade_out_seconds: 0.05,
            ..Settings::default()
        };
        engine.process(&mut [on_block], &on, &Timing::default());
        engine.process(&mut [off_block], &off, &Timing::default());

        let gain_at = |n: usize| 2.0 * (1.0 - input[n] / n as f32);
        assert!((gain_at(34) - 0.5).abs() < 1e-3, "{}", gain_at(34));
        assert!(gain_at(60).abs() < 1e-3, "{}", gain_at(60));
    }

    #[test]
    fn scope_records_the_wet_level_per_cycle_slice() {
        let mut engine = HalfSpeedEngine::new(SAMPLE_RATE, 1, 10.0);
        // Loud first half-cycle, silent second half: at 0.5x the whole cycle replays the loud
        // part, so every slot of the second cycle sees signal.
        let mut input: Vec<f32> = (0..2000)
            .map(|i| if i % 1000 < 500 { 0.8 } else { 0.0 })
            .collect();
        for block in input.chunks_mut(64) {
            engine.process(
                &mut [block],
                &settings(0.5, 1.0),
                &Timing {
                    tempo_bpm: TEMPO,
                    beat: None,
                },
            );
        }
        let scope = engine.scope();
        assert!(
            scope.iter().all(|&peak| (peak - 0.8).abs() < 1e-6),
            "{scope:?}"
        );
    }

    #[test]
    fn cycle_phase_tracks_the_loop() {
        let mut engine = HalfSpeedEngine::new(SAMPLE_RATE, 1, 10.0);
        run(&mut engine, &ramp(1250), 50, &settings(0.5, 1.0), true);
        assert!(
            (engine.cycle_phase() - 0.249).abs() < 1e-3,
            "{}",
            engine.cycle_phase()
        );
    }
}
