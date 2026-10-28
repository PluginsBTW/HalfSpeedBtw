//! Low-cut and high-cut filters for the wet signal.

use std::f64::consts::PI;

/// At or below this, the low cut is off.
pub const MIN_HZ: f32 = 20.0;
/// At or above this, the high cut is off.
pub const MAX_HZ: f32 = 20_000.0;
/// Coefficients are recalculated this often while a cutoff moves.
const UPDATE_INTERVAL: usize = 32;
/// Cutoff changes glide over roughly this time to avoid zipper noise.
const GLIDE_SECONDS: f64 = 0.02;

/// Coefficients for a 12 dB/octave Butterworth state-variable filter (Andrew Simper's
/// trapezoidal SVF). Unlike direct-form biquads, it stays smooth while its cutoff moves.
#[derive(Debug, Clone, Copy, Default)]
struct Coefs {
    a1: f32,
    a2: f32,
    a3: f32,
}

/// 1/Q for a Butterworth response.
const K: f32 = std::f32::consts::SQRT_2;

impl Coefs {
    fn new(hz: f64, sample_rate: f64) -> Self {
        let g = (PI * hz / sample_rate).tan() as f32;
        let a1 = 1.0 / (1.0 + g * (g + K));
        let a2 = g * a1;
        Self { a1, a2, a3: g * a2 }
    }
}

/// Integrator state for one filter on one channel.
#[derive(Debug, Clone, Copy, Default)]
struct State {
    ic1: f32,
    ic2: f32,
}

impl State {
    /// Returns `(low_pass, high_pass)` outputs for one input sample.
    fn run(&mut self, c: &Coefs, x: f32) -> (f32, f32) {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, x - K * v1 - v2)
    }

    /// The state after a long run of the constant `x`, so engaging the low-pass mid-signal
    /// continues from the current level instead of fading up from zero.
    fn settled_on(x: f32) -> Self {
        Self { ic1: 0.0, ic2: x }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ChannelState {
    high_pass: State,
    low_pass: State,
    /// Last input, used to settle the low-pass when it engages.
    last_input: f32,
}

/// A band-limiting filter pair: high-pass at `low_hz`, low-pass at `high_hz`. Each filter is
/// bypassed entirely while its cutoff sits at the end of the range, so the open band is
/// bit-transparent.
#[derive(Debug, Clone, Default)]
pub struct Band {
    channels: Vec<ChannelState>,
    sample_rate: f64,
    low_hz: f64,
    high_hz: f64,
    high_pass: Option<Coefs>,
    low_pass: Option<Coefs>,
    primed: bool,
    since_update: usize,
}

impl Band {
    /// Allocates state for `num_channels`. Not real-time safe.
    pub fn prepare(&mut self, sample_rate: f32, num_channels: usize) {
        self.sample_rate = sample_rate as f64;
        self.channels.clear();
        self.channels.resize(num_channels, ChannelState::default());
        self.reset();
    }

    pub fn reset(&mut self) {
        self.channels.fill(ChannelState::default());
        self.high_pass = None;
        self.low_pass = None;
        self.primed = false;
        self.since_update = 0;
    }

    /// Call once per sample frame before [`Band::process`]. Glides the cutoffs towards their
    /// targets and recalculates coefficients every few samples.
    pub fn tick(&mut self, low_hz: f32, high_hz: f32) {
        let low_target = low_hz.clamp(MIN_HZ, MAX_HZ) as f64;
        let high_target = high_hz.clamp(MIN_HZ, MAX_HZ) as f64;

        if !self.primed {
            self.low_hz = low_target;
            self.high_hz = high_target;
            self.primed = true;
        } else if self.since_update < UPDATE_INTERVAL {
            self.since_update += 1;
            return;
        } else {
            let glide =
                1.0 - (-(UPDATE_INTERVAL as f64) / (GLIDE_SECONDS * self.sample_rate)).exp();
            self.low_hz = glide_log(self.low_hz, low_target, glide);
            self.high_hz = glide_log(self.high_hz, high_target, glide);
        }
        self.since_update = 0;

        let nyquist_limit = self.sample_rate * 0.45;
        self.high_pass = (self.low_hz > MIN_HZ as f64)
            .then(|| Coefs::new(self.low_hz.min(nyquist_limit), self.sample_rate));

        let low_pass_on = self.high_hz < (MAX_HZ as f64).min(nyquist_limit);
        let engaging = low_pass_on && self.low_pass.is_none();
        self.low_pass = low_pass_on.then(|| Coefs::new(self.high_hz, self.sample_rate));
        if engaging {
            for channel in &mut self.channels {
                channel.low_pass = State::settled_on(channel.last_input);
            }
        }
    }

    /// Filters one sample of `channel`.
    pub fn process(&mut self, channel: usize, x: f32) -> f32 {
        let Some(state) = self.channels.get_mut(channel) else {
            return x;
        };
        state.last_input = x;
        let mut y = x;
        if let Some(c) = &self.high_pass {
            y = state.high_pass.run(c, y).1;
        }
        if let Some(c) = &self.low_pass {
            y = state.low_pass.run(c, y).0;
        }
        y
    }
}

/// Moves `current` towards `target` by `amount` (0..1) on a logarithmic scale, snapping once
/// close enough.
fn glide_log(current: f64, target: f64, amount: f64) -> f64 {
    let next = (current.ln() + (target.ln() - current.ln()) * amount).exp();
    if (next / target - 1.0).abs() < 1e-3 {
        target
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48_000.0;

    fn rms_through(band: &mut Band, low: f32, high: f32, freq: f32) -> f32 {
        let len = 48_000;
        let mut sum = 0.0;
        for i in 0..len {
            band.tick(low, high);
            let x = (i as f32 / SAMPLE_RATE * freq * std::f32::consts::TAU).sin();
            let y = band.process(0, x);
            // Skip the settling time.
            if i >= len / 2 {
                sum += y * y;
            }
        }
        (sum / (len / 2) as f32).sqrt()
    }

    fn band() -> Band {
        let mut band = Band::default();
        band.prepare(SAMPLE_RATE, 1);
        band
    }

    #[test]
    fn open_band_is_transparent() {
        let mut band = band();
        for i in 0..1000 {
            band.tick(MIN_HZ, MAX_HZ);
            let x = i as f32 * 0.37;
            assert_eq!(band.process(0, x), x);
        }
    }

    #[test]
    fn cuts_outside_the_band_and_passes_inside() {
        let sine_rms = std::f32::consts::FRAC_1_SQRT_2;
        let pass = rms_through(&mut band(), 300.0, 3000.0, 1000.0);
        let below = rms_through(&mut band(), 1000.0, MAX_HZ, 100.0);
        let above = rms_through(&mut band(), MIN_HZ, 1000.0, 10_000.0);
        assert!((pass / sine_rms) > 0.9, "pass {pass}");
        // 12 dB/octave over more than three octaves is well over 30 dB.
        assert!(below / sine_rms < 0.02, "below {below}");
        assert!(above / sine_rms < 0.02, "above {above}");
    }

    #[test]
    fn engaging_the_high_cut_does_not_jump() {
        let mut band = band();
        let mut previous = 0.0;
        for i in 0..4000 {
            // A slow sine, with the high cut swept in from fully open after 1000 samples.
            let high = if i < 1000 { MAX_HZ } else { 2000.0 };
            band.tick(MIN_HZ, high);
            let x = (i as f32 / SAMPLE_RATE * 50.0 * std::f32::consts::TAU).sin();
            let y = band.process(0, x);
            assert!((y - previous).abs() < 0.02, "sample {i}: {previous} -> {y}");
            previous = y;
        }
    }
}
