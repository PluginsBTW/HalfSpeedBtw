//! Colors of the PluginsBTW UI design system. There is one theme: dark.
//!
//! Translucent variants are the base color passed through `Color32::gamma_multiply`, which is
//! how the reference editor draws them; the design system writes them as `#rrggbbaa`.

use egui::Color32;

/// Window ground around the panel; label color on a selected segment.
pub const BACKGROUND: Color32 = Color32::from_rgb(0x0a, 0x0e, 0x15);
/// The main panel, and the ground for captions and knobs.
pub const SURFACE: Color32 = Color32::from_rgb(0x11, 0x18, 0x24);
/// Resting fill of controls on the panel.
pub const RAISED: Color32 = Color32::from_rgb(0x1a, 0x23, 0x33);
/// Hovered or dragged control fill, and empty arc and slider tracks.
pub const RAISED_HOVER: Color32 = Color32::from_rgb(0x22, 0x2e, 0x43);
/// A NoteKeyboard black key while its note is in the scale: lighter than every raised fill, so
/// lit black keys stand clear of the dark keys around them.
pub const KEY_BLACK: Color32 = Color32::from_rgb(0x30, 0x3a, 0x4b);
/// 1px hairlines. Decorative only: below 3:1 on `SURFACE`, so never the sole boundary cue.
pub const EDGE: Color32 = Color32::from_rgb(0x26, 0x32, 0x47);
/// The inset screen for live data.
pub const DISPLAY: Color32 = Color32::from_rgb(0x07, 0x10, 0x1f);
/// The one interactive color: anything a person has set.
pub const ACCENT: Color32 = Color32::from_rgb(0x3d, 0x9b, 0xff);
/// Live and "on" highlights: playhead, knob tip, power ring when on, hovered handles.
pub const GLOW: Color32 = Color32::from_rgb(0x5f, 0xd0, 0xff);
/// Values and primary text.
pub const TEXT: Color32 = Color32::from_rgb(0xe3, 0xea, 0xf5);
/// Captions and unselected labels. This is the design system's `text-dim-aa`, which keeps
/// 4.5:1 on every surface; HalfSpeedBtw 0.1.0 shipped the darker `#6f7d96`, which did not.
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8a, 0x95, 0xa9);

/// Accent at 15%: soft glow under active things.
pub fn accent_halo() -> Color32 {
    ACCENT.gamma_multiply(0.15)
}

/// Accent at 18%: the halo around a selected segment.
pub fn accent_halo_strong() -> Color32 {
    ACCENT.gamma_multiply(0.18)
}

/// Accent at 35%: waveform bars the playhead hasn't reached.
pub fn accent_dim() -> Color32 {
    ACCENT.gamma_multiply(0.35)
}

/// Glow at 20%: the halo behind the playhead.
pub fn glow_halo() -> Color32 {
    GLOW.gamma_multiply(0.2)
}

/// Edge at 60%: beat-grid lines inside the display.
pub fn edge_faint() -> Color32 {
    EDGE.gamma_multiply(0.6)
}

/// Background at 90%: the power switch disc over the display.
pub fn scrim() -> Color32 {
    BACKGROUND.gamma_multiply(0.9)
}

/// Opacity of everything in the display while the effect is switched off.
pub const OPACITY_OFF: f32 = 0.35;

/// WCAG 2 contrast ratio between two opaque colors, from 1 to 21.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    fn luminance(c: Color32) -> f32 {
        let channel = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.039_28 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every text color on every ground the design system puts it on.
    #[test]
    fn text_meets_wcag_aa_on_its_grounds() {
        let grounds = [BACKGROUND, SURFACE, RAISED, RAISED_HOVER, DISPLAY];
        for ground in grounds {
            for text in [TEXT, TEXT_DIM] {
                let ratio = contrast(text, ground);
                assert!(ratio >= 4.5, "{text:?} on {ground:?}: {ratio:.2}");
            }
        }
        // A selected segment's label sits on the accent fill.
        assert!(contrast(BACKGROUND, ACCENT) >= 4.5);
        // NoteKeyboard labels: on a lit black key, and on a held (glow) key.
        assert!(contrast(TEXT, KEY_BLACK) >= 4.5);
        assert!(contrast(BACKGROUND, GLOW) >= 4.5);
    }

    #[test]
    fn accent_and_glow_are_visible_marks_on_every_ground() {
        for ground in [SURFACE, RAISED, DISPLAY] {
            assert!(contrast(ACCENT, ground) >= 3.0);
            assert!(contrast(GLOW, ground) >= 3.0);
        }
    }

    #[test]
    fn translucent_variants_match_the_design_system() {
        // #rrggbbaa values from the design system's tokens.
        assert_eq!(accent_halo().a(), 0x26);
        assert_eq!(accent_halo_strong().a(), 0x2e);
        assert_eq!(accent_dim().a(), 0x59);
        assert_eq!(glow_halo().a(), 0x33);
        assert_eq!(edge_faint().a(), 0x99);
        assert_eq!(scrim().a(), 0xe6);
    }

    #[test]
    fn contrast_matches_known_values() {
        assert!((contrast(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.01);
        assert!((contrast(TEXT, SURFACE) - 14.71).abs() < 0.01);
    }
}
