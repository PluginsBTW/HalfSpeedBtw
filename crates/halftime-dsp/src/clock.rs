//! Decides where loop cycles start, either from the host's beat position or free-running.

/// Tracks the position inside the current loop cycle, one sample at a time.
///
/// In synced mode cycles are aligned to multiples of the loop length on the host's beat grid, so
/// a one-bar loop always restarts on the downbeat. Transport jumps (seeking, DAW loop wrap-around)
/// are detected and start a new cycle at the matching offset. Without a beat position the clock
/// free-runs from wherever it was, so stopping the transport doesn't cause a jump.
#[derive(Debug, Clone, Default)]
pub struct LoopClock {
    /// Samples since the start of the current cycle.
    pos: f64,
    /// Index of the current cycle on the host's beat grid, while synced.
    synced_cycle: Option<i64>,
    started: bool,
}

/// Synced positions may drift from the expected `pos + 1` by rounding error, but a larger
/// difference means the host transport jumped.
const JUMP_TOLERANCE_SAMPLES: f64 = 1.0;

impl LoopClock {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Samples since the current cycle started.
    pub fn position(&self) -> f64 {
        self.pos
    }

    /// Advances by one sample. `beat` is the host position in quarter notes at this sample, or
    /// `None` to free-run. Returns `Some(offset)` when a new cycle starts at this sample, where
    /// `offset` is how many samples into the cycle this sample already is (`0.0` on time,
    /// non-zero after a transport jump or when playback starts mid-cycle).
    pub fn tick(
        &mut self,
        loop_samples: f64,
        samples_per_beat: f64,
        beat: Option<f64>,
    ) -> Option<f64> {
        let loop_samples = loop_samples.max(1.0);

        let Some(beat) = beat else {
            self.synced_cycle = None;
            if !self.started {
                self.started = true;
                self.pos = 0.0;
                return Some(0.0);
            }

            self.pos += 1.0;
            if self.pos >= loop_samples {
                self.pos = 0.0;
                return Some(0.0);
            }
            return None;
        };

        let loop_beats = loop_samples / samples_per_beat;
        let cycle_f = (beat / loop_beats).floor();
        let cycle = cycle_f as i64;
        let offset = (beat - cycle_f * loop_beats) * samples_per_beat;

        let continuous = self.started
            && self.synced_cycle == Some(cycle)
            && (offset - (self.pos + 1.0)).abs() <= JUMP_TOLERANCE_SAMPLES;
        self.started = true;
        self.synced_cycle = Some(cycle);
        self.pos = offset;

        if continuous { None } else { Some(offset) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn restarts(
        clock: &mut LoopClock,
        beats: impl Iterator<Item = Option<f64>>,
    ) -> Vec<(usize, f64)> {
        beats
            .enumerate()
            .filter_map(|(i, beat)| clock.tick(100.0, 50.0, beat).map(|offset| (i, offset)))
            .collect()
    }

    #[test]
    fn free_running_restarts_every_loop() {
        let mut clock = LoopClock::default();
        let events = restarts(&mut clock, (0..250).map(|_| None));
        assert_eq!(events, [(0, 0.0), (100, 0.0), (200, 0.0)]);
    }

    #[test]
    fn synced_restarts_on_the_beat_grid() {
        // 50 samples per beat, 100-sample loop = 2 beats. Playback starts at beat 1.0, halfway
        // through the first cycle.
        let mut clock = LoopClock::default();
        let beats = (0..250).map(|i| Some(1.0 + i as f64 / 50.0));
        let events = restarts(&mut clock, beats);
        assert_eq!(events, [(0, 50.0), (50, 0.0), (150, 0.0)]);
    }

    #[test]
    fn synced_detects_transport_jumps() {
        let mut clock = LoopClock::default();
        // Plays beats 0.0..1.0, then the host loops back to beat 0.5 (same cycle, earlier).
        let first = (0..50).map(|i| Some(i as f64 / 50.0));
        let second = (0..10).map(|i| Some(0.5 + i as f64 / 50.0));
        let events = restarts(&mut clock, first.chain(second));
        assert_eq!(events, [(0, 0.0), (50, 25.0)]);
    }

    #[test]
    fn stopping_the_transport_continues_without_restart() {
        let mut clock = LoopClock::default();
        let playing = (0..30).map(|i| Some(i as f64 / 50.0));
        let stopped = (0..80).map(|_| None);
        // Free-running continues from position 29 and wraps at 100.
        let events = restarts(&mut clock, playing.chain(stopped));
        assert_eq!(events, [(0, 0.0), (100, 0.0)]);
    }
}
