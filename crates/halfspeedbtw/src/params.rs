//! Host-visible parameters.
//!
//! The `#[id]` strings, on parameters and on enum variants, are stored in DAW projects and
//! automation lanes. Never rename or reuse one once it has shipped; add new IDs instead.

use halfspeed_dsp::Settings;
use halfspeed_dsp::band::{MAX_HZ, MIN_HZ};
use nice_plug::prelude::*;

/// Fade lengths selectable on the fade in/out knobs, in bars. Index 0 is instant.
pub const FADE_BARS: [f64; 10] = [
    0.0,
    1.0 / 16.0,
    1.0 / 8.0,
    0.25,
    0.5,
    1.0,
    2.0,
    4.0,
    8.0,
    16.0,
];
const FADE_LABELS: [&str; 10] = [
    "Inst", "1/16", "1/8", "1/4", "1/2", "1 Bar", "2 Bars", "4 Bars", "8 Bars", "16 Bars",
];

#[derive(Params)]
pub struct HalfSpeedParams {
    /// Turns the slowdown on or off, fading over the fade in/out lengths. Distinct from the
    /// host's bypass: when off, the loop keeps running so turning it back on stays in sync.
    #[id = "enabled"]
    pub enabled: BoolParam,

    /// Dry/wet balance between the input and the slowed-down signal.
    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "ratio"]
    pub ratio: EnumParam<Mode>,

    #[id = "loop_length"]
    pub loop_length: EnumParam<LoopLength>,

    /// How long the effect takes to fade in after switching on. Index into [`FADE_BARS`].
    #[id = "fade_in"]
    pub fade_in: IntParam,

    /// How long the effect takes to fade out after switching off. Index into [`FADE_BARS`].
    #[id = "fade_out"]
    pub fade_out: IntParam,

    /// Crossfade length at each loop restart, from barely-there to a quarter of the loop.
    #[id = "smoothness"]
    pub smoothness: FloatParam,

    /// High-pass cutoff on the slowed-down signal.
    #[id = "low_cut"]
    pub low_cut: FloatParam,

    /// Low-pass cutoff on the slowed-down signal.
    #[id = "high_cut"]
    pub high_cut: FloatParam,

    /// Aligns loops to the host's bar grid while the transport plays. When off, or while the
    /// transport is stopped, loops free-run at the host tempo.
    #[id = "sync"]
    pub sync: BoolParam,
}

/// How much the loop is slowed down.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    #[id = "two_thirds"]
    #[name = "1.5x"]
    OneAndAHalf,
    #[id = "half"]
    #[name = "2x"]
    Two,
    #[id = "quarter"]
    #[name = "4x"]
    Four,
}

impl Mode {
    /// Playback speed relative to the input.
    pub fn ratio(self) -> f64 {
        match self {
            Self::OneAndAHalf => 2.0 / 3.0,
            Self::Two => 0.5,
            Self::Four => 0.25,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopLength {
    #[id = "1/16"]
    #[name = "1/16"]
    Sixteenth,
    #[id = "1/8t"]
    #[name = "1/8T"]
    EighthTriplet,
    #[id = "1/8"]
    #[name = "1/8"]
    Eighth,
    #[id = "1/4"]
    #[name = "1/4"]
    Quarter,
    #[id = "1/2"]
    #[name = "1/2"]
    Half,
    #[id = "1"]
    #[name = "1 Bar"]
    One,
    #[id = "2"]
    #[name = "2 Bars"]
    Two,
    #[id = "4"]
    #[name = "4 Bars"]
    Four,
    #[id = "8"]
    #[name = "8 Bars"]
    Eight,
}

impl LoopLength {
    pub fn bars(self) -> f64 {
        match self {
            Self::Sixteenth => 1.0 / 16.0,
            // Three triplet eighths fill a quarter bar.
            Self::EighthTriplet => 1.0 / 12.0,
            Self::Eighth => 1.0 / 8.0,
            Self::Quarter => 1.0 / 4.0,
            Self::Half => 1.0 / 2.0,
            Self::One => 1.0,
            Self::Two => 2.0,
            Self::Four => 4.0,
            Self::Eight => 8.0,
        }
    }
}

impl HalfSpeedParams {
    /// Engine settings for the current parameter values. `beats_per_bar` comes from the host's
    /// time signature, in quarter notes.
    pub fn settings(&self, beats_per_bar: f64, tempo_bpm: f64) -> Settings {
        let seconds_per_bar = beats_per_bar * 60.0 / tempo_bpm;
        let fade_seconds = |param: &IntParam| fade_seconds(param.value(), seconds_per_bar);
        Settings {
            enabled: self.enabled.value(),
            ratio: self.ratio.value().ratio(),
            loop_beats: self.loop_length.value().bars() * beats_per_bar,
            smoothness: self.smoothness.value(),
            fade_in_seconds: fade_seconds(&self.fade_in),
            fade_out_seconds: fade_seconds(&self.fade_out),
            low_cut_hz: self.low_cut.value(),
            high_cut_hz: self.high_cut.value(),
            mix: self.mix.value(),
        }
    }
}

/// Duration of the fade at `index` into [`FADE_BARS`].
fn fade_seconds(index: i32, seconds_per_bar: f64) -> f32 {
    let bars = FADE_BARS[index.clamp(0, FADE_BARS.len() as i32 - 1) as usize];
    (bars * seconds_per_bar) as f32
}

fn frequency_param(name: &str, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: MIN_HZ,
            max: MAX_HZ,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
    .with_string_to_value(formatters::s2v_f32_hz_then_khz())
}

fn fade_param(name: &str, default: i32) -> IntParam {
    IntParam::new(
        name,
        default,
        IntRange::Linear {
            min: 0,
            max: FADE_BARS.len() as i32 - 1,
        },
    )
    .with_value_to_string(std::sync::Arc::new(|index| {
        FADE_LABELS[index.clamp(0, FADE_LABELS.len() as i32 - 1) as usize].to_string()
    }))
}

impl Default for HalfSpeedParams {
    fn default() -> Self {
        Self {
            enabled: BoolParam::new("Effect", true),
            // Smoothing happens in the engine, which ramps mix changes per sample.
            mix: FloatParam::new("Mix", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit("%")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            ratio: EnumParam::new("Mode", Mode::Two),
            loop_length: EnumParam::new("Loop Length", LoopLength::One),
            fade_in: fade_param("Fade In", 0),
            fade_out: fade_param("Fade Out", 0),
            smoothness: FloatParam::new(
                "Smoothness",
                0.3,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
            low_cut: frequency_param("Low Cut", MIN_HZ),
            high_cut: frequency_param("High Cut", MAX_HZ),
            sync: BoolParam::new("Sync", true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards against accidental renames: changing an ID silently breaks saved projects.
    #[test]
    fn parameter_ids_are_stable() {
        let params = HalfSpeedParams::default();
        let ids: Vec<String> = params.param_map().into_iter().map(|(id, ..)| id).collect();
        assert_eq!(
            ids,
            [
                "enabled",
                "mix",
                "ratio",
                "loop_length",
                "fade_in",
                "fade_out",
                "smoothness",
                "low_cut",
                "high_cut",
                "sync",
            ]
        );
    }

    #[test]
    fn enum_variant_ids_are_stable() {
        assert_eq!(Mode::ids(), Some(&["two_thirds", "half", "quarter"][..]));
        assert_eq!(
            LoopLength::ids(),
            Some(&["1/16", "1/8t", "1/8", "1/4", "1/2", "1", "2", "4", "8"][..])
        );
    }

    #[test]
    fn musical_lengths_follow_tempo_and_time_signature() {
        let params = HalfSpeedParams::default();
        // One bar of 4/4 is four quarter notes, one bar of 6/8 is three.
        assert_eq!(params.settings(4.0, 120.0).loop_beats, 4.0);
        assert_eq!(params.settings(3.0, 120.0).loop_beats, 3.0);
        assert_eq!(LoopLength::EighthTriplet.bars() * 4.0, 1.0 / 3.0);

        // Fade lengths are in bars: 1 bar of 4/4 at 120 BPM lasts 2 seconds.
        assert_eq!(fade_seconds(0, 2.0), 0.0);
        assert_eq!(fade_seconds(5, 2.0), 2.0);
        assert_eq!(fade_seconds(99, 2.0), 32.0);
    }

    #[test]
    fn fade_labels_cover_every_step() {
        let params = HalfSpeedParams::default();
        assert_eq!(
            params.fade_in.normalized_value_to_string(0.0, false),
            "Inst"
        );
        assert_eq!(
            params.fade_in.normalized_value_to_string(1.0, false),
            "16 Bars"
        );
    }
}
