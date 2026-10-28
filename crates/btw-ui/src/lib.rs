//! The PluginsBTW UI design system for nice-plug editors built with egui.
//!
//! Plugin editors are laid out absolutely on a fixed-size window and drawn with egui's
//! painter: a dark `background`, one `surface` panel, controls on `raised` fills, `accent`
//! for anything a person has set and `glow` for anything live. [`palette`] holds the colors,
//! [`type_scale`] the five font sizes, and [`widgets`] the controls, each of which talks to
//! the host through a nice-plug [`ParamSetter`](nice_plug::prelude::ParamSetter).
//!
//! The reference editor is `crates/halfspeedbtw/src/editor.rs`; the full guide, with usage
//! rules and previews, is the "PluginsBTW UI" design system.

pub mod geometry;
pub mod palette;
pub mod widgets;

/// The only font sizes the system uses, in logical pixels. All text is egui's built-in
/// proportional face (Ubuntu Light).
pub mod type_scale {
    /// The product name in the header.
    pub const WORDMARK: f32 = 22.0;
    /// Labels inside segmented controls.
    pub const CONTROL_LABEL: f32 = 13.0;
    /// The value inside a large knob.
    pub const VALUE_LARGE: f32 = 13.0;
    /// The value inside a small knob.
    pub const VALUE: f32 = 12.0;
    /// Uppercase captions, slider readouts and the status line.
    pub const CAPTION: f32 = 11.0;
}
