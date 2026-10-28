//! Host-visible parameters.
//!
//! The `#[id]` strings are stored in DAW projects and automation lanes. Never rename or reuse
//! one once it has shipped; add new IDs instead.

use nice_plug::prelude::*;

#[derive(Params)]
pub struct HalfTimeParams {
    /// Turns the half-speed effect on or off. Distinct from the host's bypass: when off, the
    /// plugin still runs and can crossfade back to the dry signal.
    #[id = "enabled"]
    pub enabled: BoolParam,

    /// Dry/wet balance between the input and the half-speed signal.
    #[id = "mix"]
    pub mix: FloatParam,
}

impl Default for HalfTimeParams {
    fn default() -> Self {
        Self {
            enabled: BoolParam::new("Effect", true),
            mix: FloatParam::new("Mix", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_unit("%")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
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
        assert_eq!(ids, ["enabled", "mix"]);
    }
}
