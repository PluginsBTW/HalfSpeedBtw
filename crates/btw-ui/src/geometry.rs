//! Pure geometry and color helpers behind the widgets.

use std::f32::consts::PI;

use egui::{Color32, Pos2, Vec2};

/// Knob arcs start at the bottom left...
pub const KNOB_START: f32 = 0.75 * PI;
/// ...and sweep 270° clockwise, leaving a gap at the bottom.
pub const KNOB_SWEEP: f32 = 1.5 * PI;

/// Points along a circular arc, clockwise on screen from `start` over `sweep` radians.
pub fn arc_points(center: Pos2, radius: f32, start: f32, sweep: f32, segments: usize) -> Vec<Pos2> {
    let segments = segments.max(1);
    (0..=segments)
        .map(|i| center + radius * Vec2::angled(start + sweep * i as f32 / segments as f32))
        .collect()
}

/// Mixes two opaque colors; `t` = 0 gives `a`, 1 gives `b`.
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// Maps values between `min` and `max` logarithmically onto a span of pixels, as the range
/// slider does for frequencies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogScale {
    pub min: f32,
    pub max: f32,
}

impl LogScale {
    pub const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    /// Position of `value` as a fraction `0..=1` of the span, clamped to the range.
    pub fn fraction_of(&self, value: f32) -> f32 {
        (value.clamp(self.min, self.max) / self.min).ln() / (self.max / self.min).ln()
    }

    /// The value at `fraction` (clamped to `0..=1`) of the span.
    pub fn value_at(&self, fraction: f32) -> f32 {
        self.min * (fraction.clamp(0.0, 1.0) * (self.max / self.min).ln()).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knob_arc_runs_from_bottom_left_to_bottom_right() {
        let points = arc_points(Pos2::ZERO, 10.0, KNOB_START, KNOB_SWEEP, 48);
        assert_eq!(points.len(), 49);
        let (first, last) = (points[0], points[48]);
        // Screen y grows downwards, so both ends sit below the center, mirrored.
        assert!(first.x < 0.0 && first.y > 0.0, "{first:?}");
        assert!(last.x > 0.0 && last.y > 0.0, "{last:?}");
        assert!((first.x + last.x).abs() < 1e-4 && (first.y - last.y).abs() < 1e-4);
        // Halfway round is straight up.
        assert!((points[24] - Pos2::new(0.0, -10.0)).length() < 1e-4);
    }

    #[test]
    fn log_scale_round_trips_and_places_decades_evenly() {
        let scale = LogScale::new(20.0, 20_000.0);
        assert_eq!(scale.fraction_of(20.0), 0.0);
        assert!((scale.fraction_of(20_000.0) - 1.0).abs() < 1e-6);
        assert!((scale.fraction_of(200.0) - 1.0 / 3.0).abs() < 1e-6);
        assert!((scale.fraction_of(2000.0) - 2.0 / 3.0).abs() < 1e-6);
        for hz in [25.0, 180.0, 1000.0, 6500.0] {
            assert!((scale.value_at(scale.fraction_of(hz)) / hz - 1.0).abs() < 1e-4);
        }
        assert_eq!(scale.fraction_of(5.0), 0.0);
        assert_eq!(scale.value_at(2.0), scale.value_at(1.0));
    }

    #[test]
    fn lerp_color_hits_both_ends() {
        let (a, b) = (
            Color32::from_rgb(0, 100, 200),
            Color32::from_rgb(200, 100, 0),
        );
        assert_eq!(lerp_color(a, b, 0.0), a);
        assert_eq!(lerp_color(a, b, 1.0), b);
        assert_eq!(lerp_color(a, b, 0.5), Color32::from_rgb(100, 100, 100));
        assert_eq!(lerp_color(a, b, 7.0), b);
    }
}
