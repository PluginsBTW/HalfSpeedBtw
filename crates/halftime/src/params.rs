//! Host-visible parameters.
//!
//! The `#[id]` strings, on parameters and on enum variants, are stored in DAW projects and
//! automation lanes. Never rename or reuse one once it has shipped; add new IDs instead.

use halftime_dsp::Settings;
use nice_plug::prelude::*;

#[derive(Params)]
pub struct HalfTimeParams {
    /// Turns the half-speed effect on or off, fading over `fade`. Distinct from the host's
    /// bypass: when off, the loop keeps running so turning it back on stays in sync.
    #[id = "enabled"]
    pub enabled: BoolParam,

    /// Dry/wet balance between the input and the half-speed signal.
    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "ratio"]
    pub ratio: EnumParam<PlaybackRatio>,

    #[id = "loop_length"]
    pub loop_length: EnumParam<LoopLength>,

    /// How quickly the effect fades in and out when it is switched.
    #[id = "fade"]
    pub fade: EnumParam<Fade>,

    /// Crossfade length at each loop restart, from barely-there to a quarter of the loop.
    #[id = "smoothness"]
    pub smoothness: FloatParam,

    /// Aligns loops to the host's bar grid while the transport plays. When off, or while the
    /// transport is stopped, loops free-run at the host tempo.
    #[id = "sync"]
    pub sync: BoolParam,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackRatio {
    #[id = "half"]
    #[name = "0.5x"]
    Half,
    #[id = "two_thirds"]
    #[name = "0.667x"]
    TwoThirds,
    #[id = "quarter"]
    #[name = "0.25x"]
    Quarter,
}

impl PlaybackRatio {
    pub fn ratio(self) -> f64 {
        match self {
            Self::Half => 0.5,
            Self::TwoThirds => 2.0 / 3.0,
            Self::Quarter => 0.25,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopLength {
    #[id = "1/16"]
    #[name = "1/16 bar"]
    Sixteenth,
    #[id = "1/8"]
    #[name = "1/8 bar"]
    Eighth,
    #[id = "1/4"]
    #[name = "1/4 bar"]
    Quarter,
    #[id = "1/2"]
    #[name = "1/2 bar"]
    Half,
    #[id = "1"]
    #[name = "1 bar"]
    One,
    #[id = "2"]
    #[name = "2 bars"]
    Two,
    #[id = "4"]
    #[name = "4 bars"]
    Four,
}

impl LoopLength {
    pub fn bars(self) -> f64 {
        match self {
            Self::Sixteenth => 1.0 / 16.0,
            Self::Eighth => 1.0 / 8.0,
            Self::Quarter => 1.0 / 4.0,
            Self::Half => 1.0 / 2.0,
            Self::One => 1.0,
            Self::Two => 2.0,
            Self::Four => 4.0,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fade {
    #[id = "off"]
    Off,
    #[id = "fast"]
    Fast,
    #[id = "normal"]
    Normal,
    #[id = "slow"]
    Slow,
}

impl Fade {
    pub fn seconds(self) -> f32 {
        match self {
            Self::Off => 0.0,
            Self::Fast => 0.05,
            Self::Normal => 0.25,
            Self::Slow => 1.0,
        }
    }
}

impl HalfTimeParams {
    /// Engine settings for the current parameter values. `beats_per_bar` comes from the host's
    /// time signature, in quarter notes.
    pub fn settings(&self, beats_per_bar: f64) -> Settings {
        Settings {
            enabled: self.enabled.value(),
            ratio: self.ratio.value().ratio(),
            loop_beats: self.loop_length.value().bars() * beats_per_bar,
            smoothness: self.smoothness.value(),
            fade_seconds: self.fade.value().seconds(),
            mix: self.mix.value(),
        }
    }
}

impl Default for HalfTimeParams {
    fn default() -> Self {
        Self {
            enabled: BoolParam::new("Effect", true),
            // Smoothing happens in the engine, which ramps mix changes per sample.
            mix: FloatParam::new("Mix", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit("%")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            ratio: EnumParam::new("Speed", PlaybackRatio::Half),
            loop_length: EnumParam::new("Loop Length", LoopLength::One),
            fade: EnumParam::new("Fade", Fade::Fast),
            smoothness: FloatParam::new(
                "Smoothness",
                0.3,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
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
        let params = HalfTimeParams::default();
        let ids: Vec<String> = params.param_map().into_iter().map(|(id, ..)| id).collect();
        assert_eq!(
            ids,
            [
                "enabled",
                "mix",
                "ratio",
                "loop_length",
                "fade",
                "smoothness",
                "sync"
            ]
        );
    }

    #[test]
    fn enum_variant_ids_are_stable() {
        assert_eq!(
            PlaybackRatio::ids(),
            Some(&["half", "two_thirds", "quarter"][..])
        );
        assert_eq!(
            LoopLength::ids(),
            Some(&["1/16", "1/8", "1/4", "1/2", "1", "2", "4"][..])
        );
        assert_eq!(Fade::ids(), Some(&["off", "fast", "normal", "slow"][..]));
    }

    #[test]
    fn loop_length_follows_the_time_signature() {
        let params = HalfTimeParams::default();
        // One bar of 4/4 is four quarter notes, one bar of 6/8 is three.
        assert_eq!(params.settings(4.0).loop_beats, 4.0);
        assert_eq!(params.settings(3.0).loop_beats, 3.0);
    }
}
