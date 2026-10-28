//! The HalfSpeedBtw editor: a dark, blue-accented panel with loop buttons on top, a waveform
//! display with the power switch in the middle flanked by fade knobs, mode buttons below it,
//! and smoothness, band and mix controls along the bottom.
//!
//! Everything is laid out absolutely on a fixed-size window and drawn with egui's painter, so
//! the widgets here are small hand-rolled controls rather than stock egui widgets.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use egui::{
    Align2, Color32, CursorIcon, FontId, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui,
    Vec2, pos2, vec2,
};
use halfspeed_dsp::band::{MAX_HZ, MIN_HZ};
use nice_plug::prelude::{Enum, EnumParam, FloatParam, Param, ParamSetter};
use nice_plug::{context::gui::GuiContext, editor::dpi::LogicalSize};
use nice_plug_egui::NiceEguiApp;

use crate::DisplayState;
use crate::params::HalfSpeedParams;

pub const WINDOW_SIZE: LogicalSize<f32> = LogicalSize::new(680.0, 480.0);

const BACKGROUND: Color32 = Color32::from_rgb(0x0a, 0x0e, 0x15);
const SURFACE: Color32 = Color32::from_rgb(0x11, 0x18, 0x24);
const RAISED: Color32 = Color32::from_rgb(0x1a, 0x23, 0x33);
const RAISED_HOVER: Color32 = Color32::from_rgb(0x22, 0x2e, 0x43);
const EDGE: Color32 = Color32::from_rgb(0x26, 0x32, 0x47);
const DISPLAY: Color32 = Color32::from_rgb(0x07, 0x10, 0x1f);
const ACCENT: Color32 = Color32::from_rgb(0x3d, 0x9b, 0xff);
const GLOW: Color32 = Color32::from_rgb(0x5f, 0xd0, 0xff);
const TEXT: Color32 = Color32::from_rgb(0xe3, 0xea, 0xf5);
const DIM_TEXT: Color32 = Color32::from_rgb(0x6f, 0x7d, 0x96);

/// Knob arcs run clockwise from bottom-left to bottom-right, leaving a gap at the bottom.
const KNOB_START: f32 = 0.75 * PI;
const KNOB_SWEEP: f32 = 1.5 * PI;
/// Normalized change per pixel of knob drag; Shift drags ten times finer.
const DRAG_SPEED: f32 = 0.005;

pub struct HalfSpeedEditor {
    params: Arc<HalfSpeedParams>,
    display: Arc<DisplayState>,
    gui_ctx: Option<GuiContext>,
}

impl HalfSpeedEditor {
    pub fn new(params: Arc<HalfSpeedParams>, display: Arc<DisplayState>) -> Self {
        Self {
            params,
            display,
            gui_ctx: None,
        }
    }
}

impl NiceEguiApp for HalfSpeedEditor {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        _frame: &mut nice_plug_egui::Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        egui_ctx.set_visuals(egui::Visuals::dark());
        self.gui_ctx = Some(nice_gui_ctx);
        Ok(())
    }

    fn editor_closed(&mut self) {
        self.gui_ctx = None;
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut nice_plug_egui::Frame) {
        let Some(gui_ctx) = self.gui_ctx.as_ref() else {
            return;
        };
        let setter = gui_ctx.param_setter();
        let params = &*self.params;
        let painter = ui.painter().clone();
        let origin = ui.max_rect().min;
        let at =
            |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(origin + vec2(x, y), vec2(w, h));
        let point = |x: f32, y: f32| origin + vec2(x, y);

        painter.rect_filled(ui.max_rect(), 0, BACKGROUND);
        let panel = at(12.0, 12.0, 656.0, 456.0);
        painter.rect_filled(panel, 14, SURFACE);
        painter.rect_stroke(panel, 14, Stroke::new(1.0, EDGE), StrokeKind::Inside);

        header(ui, &painter, &setter, params, &self.display, origin);

        segmented(
            ui,
            &painter,
            &setter,
            "loop",
            at(32.0, 64.0, 616.0, 34.0),
            &params.loop_length,
        );
        caption(&painter, point(340.0, 112.0), "LOOP");

        knob(
            ui,
            &painter,
            &setter,
            "fade_in",
            point(95.0, 205.0),
            38.0,
            &params.fade_in,
            "FADE IN",
        );
        waveform(
            ui,
            &painter,
            &setter,
            params,
            &self.display,
            at(170.0, 128.0, 340.0, 166.0),
        );
        knob(
            ui,
            &painter,
            &setter,
            "fade_out",
            point(585.0, 205.0),
            38.0,
            &params.fade_out,
            "FADE OUT",
        );

        segmented(
            ui,
            &painter,
            &setter,
            "mode",
            at(230.0, 314.0, 220.0, 32.0),
            &params.ratio,
        );
        caption(&painter, point(340.0, 360.0), "MODE");

        knob(
            ui,
            &painter,
            &setter,
            "smooth",
            point(95.0, 405.0),
            32.0,
            &params.smoothness,
            "SMOOTH",
        );
        band(ui, &painter, &setter, params, at(190.0, 386.0, 300.0, 50.0));
        knob(
            ui,
            &painter,
            &setter,
            "mix",
            point(585.0, 405.0),
            32.0,
            &params.mix,
            "MIX",
        );
    }
}

/// Logo, name, sync switch and engine status.
fn header(
    ui: &mut Ui,
    painter: &Painter,
    setter: &ParamSetter,
    params: &HalfSpeedParams,
    display: &DisplayState,
    origin: Pos2,
) {
    // A slowed-down clock: a ring with one long, lazy hand.
    let logo = origin + vec2(42.0, 38.0);
    painter.circle_stroke(logo, 11.0, Stroke::new(2.5, ACCENT));
    painter.line_segment([logo, logo + vec2(0.0, -7.0)], Stroke::new(2.5, GLOW));
    painter.line_segment([logo, logo + vec2(5.0, 3.0)], Stroke::new(2.5, GLOW));

    let font = FontId::proportional(22.0);
    let mut x = logo.x + 20.0;
    for (word, color) in [("Half", TEXT), ("Speed", ACCENT), ("Btw", DIM_TEXT)] {
        let rect = painter.text(
            pos2(x, logo.y),
            Align2::LEFT_CENTER,
            word,
            font.clone(),
            color,
        );
        x = rect.right();
    }

    let sample_rate = display.sample_rate.load(Ordering::Relaxed);
    painter.text(
        origin + vec2(540.0, 38.0),
        Align2::RIGHT_CENTER,
        format!("{:.1} kHz · 0 latency", sample_rate / 1000.0),
        FontId::proportional(11.0),
        DIM_TEXT,
    );

    let sync = params.sync.value();
    let pill = Rect::from_min_size(origin + vec2(556.0, 25.0), vec2(92.0, 26.0));
    let response = ui
        .interact(pill, ui.scope_id().with("sync"), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Follow the host's bar grid while playing");
    if response.clicked() {
        set(setter, &params.sync, !sync);
    }
    let (fill, stroke, text) = if sync {
        (ACCENT.gamma_multiply(0.15), ACCENT, GLOW)
    } else {
        (RAISED, EDGE, DIM_TEXT)
    };
    painter.rect_filled(pill, 13, fill);
    painter.rect_stroke(pill, 13, Stroke::new(1.0, stroke), StrokeKind::Inside);
    painter.text(
        pill.center(),
        Align2::CENTER_CENTER,
        if sync { "SYNC ON" } else { "SYNC OFF" },
        FontId::proportional(11.0),
        text,
    );
}

/// The loop's waveform with a playhead, and the power switch on top of it.
fn waveform(
    ui: &mut Ui,
    painter: &Painter,
    setter: &ParamSetter,
    params: &HalfSpeedParams,
    display: &DisplayState,
    rect: Rect,
) {
    let enabled = params.enabled.value();
    let shade = |color: Color32| {
        if enabled {
            color
        } else {
            color.gamma_multiply(0.35)
        }
    };

    painter.rect_filled(rect, 12, DISPLAY);
    painter.rect_stroke(rect, 12, Stroke::new(1.0, EDGE), StrokeKind::Inside);

    let inner = rect.shrink2(vec2(14.0, 16.0));
    let mid_y = inner.center().y;
    for quarter in 1..4 {
        let x = inner.left() + inner.width() * quarter as f32 / 4.0;
        painter.line_segment(
            [pos2(x, inner.top()), pos2(x, inner.bottom())],
            Stroke::new(1.0, EDGE.gamma_multiply(0.6)),
        );
    }

    let phase = display.cycle_phase.load(Ordering::Relaxed);
    let bins = display.scope.len();
    let bin_width = inner.width() / bins as f32;
    for (i, slot) in display.scope.iter().enumerate() {
        // Square root lifts quiet material so the shape stays readable.
        let level = slot.load(Ordering::Relaxed).max(0.0).sqrt().min(1.0);
        let half_height = (level * inner.height() * 0.5).max(0.5);
        let x = inner.left() + i as f32 * bin_width;
        let played = (i as f32 + 0.5) / bins as f32 <= phase;
        let color = if played {
            lerp_color(ACCENT, GLOW, level)
        } else {
            ACCENT.gamma_multiply(0.35)
        };
        painter.rect_filled(
            Rect::from_min_max(
                pos2(x, mid_y - half_height),
                pos2(x + bin_width * 0.8, mid_y + half_height),
            ),
            0,
            shade(color),
        );
    }

    let playhead_x = inner.left() + phase * inner.width();
    let playhead = [
        pos2(playhead_x, rect.top() + 6.0),
        pos2(playhead_x, rect.bottom() - 6.0),
    ];
    painter.line_segment(playhead, Stroke::new(5.0, shade(GLOW.gamma_multiply(0.2))));
    painter.line_segment(playhead, Stroke::new(1.5, shade(GLOW)));

    // Power switch.
    let center = rect.center();
    let radius = 30.0;
    let response = ui
        .interact(
            Rect::from_center_size(center, Vec2::splat(2.0 * radius)),
            ui.scope_id().with("power"),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    if response.clicked() {
        set(setter, &params.enabled, !enabled);
    }
    let ring = match (enabled, response.hovered()) {
        (true, _) => GLOW,
        (false, true) => TEXT,
        (false, false) => DIM_TEXT,
    };
    if enabled {
        painter.circle_filled(center, radius + 6.0, ACCENT.gamma_multiply(0.15));
    }
    painter.circle_filled(center, radius, BACKGROUND.gamma_multiply(0.9));
    painter.circle_stroke(center, radius, Stroke::new(2.0, ring));
    let gap = 0.7;
    let icon = arc_points(center, 12.0, -PI / 2.0 + gap, TAU - 2.0 * gap, 32);
    painter.add(Shape::line(icon, Stroke::new(3.0, ring)));
    painter.line_segment(
        [center + vec2(0.0, -16.0), center + vec2(0.0, -4.0)],
        Stroke::new(3.0, ring),
    );
}

/// A row of equally sized buttons, one per enum variant.
fn segmented<T: Enum + PartialEq + 'static>(
    ui: &mut Ui,
    painter: &Painter,
    setter: &ParamSetter,
    id: &str,
    rect: Rect,
    param: &EnumParam<T>,
) {
    let names = T::variants();
    let gap = 6.0;
    let width = (rect.width() - gap * (names.len() - 1) as f32) / names.len() as f32;
    let current = param.value().to_index();

    for (index, name) in names.iter().enumerate() {
        let button = Rect::from_min_size(
            rect.min + vec2(index as f32 * (width + gap), 0.0),
            vec2(width, rect.height()),
        );
        let response = ui
            .interact(button, ui.scope_id().with(id).with(index), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let selected = index == current;
        if response.clicked() && !selected {
            set(setter, param, T::from_index(index));
        }

        if selected {
            painter.rect_filled(button.expand(3.0), 10, ACCENT.gamma_multiply(0.18));
            painter.rect_filled(button, 7, ACCENT);
        } else {
            let fill = if response.hovered() {
                RAISED_HOVER
            } else {
                RAISED
            };
            painter.rect_filled(button, 7, fill);
            painter.rect_stroke(button, 7, Stroke::new(1.0, EDGE), StrokeKind::Inside);
        }
        let text = match (selected, response.hovered()) {
            (true, _) => BACKGROUND,
            (false, true) => TEXT,
            (false, false) => DIM_TEXT,
        };
        painter.text(
            button.center(),
            Align2::CENTER_CENTER,
            *name,
            FontId::proportional(13.0),
            text,
        );
    }
}

/// A ring knob showing its value in the middle. Drag vertically (Shift for fine control),
/// double-click to reset.
#[allow(clippy::too_many_arguments)]
fn knob<P: Param>(
    ui: &mut Ui,
    painter: &Painter,
    setter: &ParamSetter,
    id: &str,
    center: Pos2,
    radius: f32,
    param: &P,
    label: &str,
) {
    let id = ui.scope_id().with(id);
    let rect = Rect::from_center_size(center, Vec2::splat(2.0 * radius + 8.0));
    let response = ui
        .interact(rect, id, Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::ResizeVertical);

    // Stepped parameters snap on every set, so accumulate the drag separately.
    if response.drag_started() {
        setter.begin_set_parameter(param);
        let start = param.unmodulated_normalized_value();
        ui.data_mut(|data| data.insert_temp(id, start));
    }
    if response.dragged() {
        let fine = ui.input(|input| input.modifiers.shift);
        let speed = if fine { DRAG_SPEED / 10.0 } else { DRAG_SPEED };
        let delta = response.drag_delta();
        let value = ui.data_mut(|data| {
            let value = data.get_temp_mut_or(id, param.unmodulated_normalized_value());
            *value = (*value + (delta.x - delta.y) * speed).clamp(0.0, 1.0);
            *value
        });
        setter.set_parameter_normalized(param, value);
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
    if response.double_clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter_normalized(param, param.default_normalized_value());
        setter.end_set_parameter(param);
    }

    let value = param.unmodulated_normalized_value();
    let track = Stroke::new(5.0, RAISED_HOVER);
    painter.add(Shape::line(
        arc_points(center, radius, KNOB_START, KNOB_SWEEP, 48),
        track,
    ));
    if value > 0.0 {
        let arc = arc_points(center, radius, KNOB_START, KNOB_SWEEP * value, 48);
        painter.add(Shape::line(
            arc.clone(),
            Stroke::new(9.0, ACCENT.gamma_multiply(0.15)),
        ));
        painter.add(Shape::line(arc, Stroke::new(5.0, ACCENT)));
    }
    let tip = center + radius * Vec2::angled(KNOB_START + KNOB_SWEEP * value);
    painter.circle_filled(tip, 4.0, GLOW);

    let body = if response.hovered() || response.dragged() {
        RAISED_HOVER
    } else {
        RAISED
    };
    painter.circle_filled(center, radius - 9.0, body);
    painter.circle_stroke(center, radius - 9.0, Stroke::new(1.0, EDGE));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        param.normalized_value_to_string(value, true),
        FontId::proportional(if radius > 35.0 { 13.0 } else { 12.0 }),
        TEXT,
    );
    caption(painter, center + vec2(0.0, radius + 16.0), label);
}

/// A two-handled range slider for the low and high cut, on a logarithmic frequency scale.
fn band(
    ui: &mut Ui,
    painter: &Painter,
    setter: &ParamSetter,
    params: &HalfSpeedParams,
    rect: Rect,
) {
    let track_y = rect.top() + 18.0;
    let left = rect.left() + 6.0;
    let width = rect.width() - 12.0;
    let span = (MAX_HZ / MIN_HZ).ln();
    let x_of = |hz: f32| left + (hz.clamp(MIN_HZ, MAX_HZ) / MIN_HZ).ln() / span * width;
    let hz_of = |x: f32| MIN_HZ * (((x - left) / width).clamp(0.0, 1.0) * span).exp();

    let low = params.low_cut.value();
    let high = params.high_cut.value();
    let track = Rect::from_min_max(pos2(left, track_y - 3.0), pos2(left + width, track_y + 3.0));
    painter.rect_filled(track, 3, RAISED_HOVER);
    let active = Rect::from_min_max(
        pos2(x_of(low), track.top()),
        pos2(x_of(high), track.bottom()),
    );
    painter.rect_filled(active.expand(3.0), 6, ACCENT.gamma_multiply(0.15));
    painter.rect_filled(active, 3, ACCENT);

    // Keep the handles from crossing so the band never closes completely.
    let min_ratio = 1.25;
    for (id, param, is_low) in [
        ("low_cut", &params.low_cut, true),
        ("high_cut", &params.high_cut, false),
    ] {
        let x = x_of(param.value());
        let handle = Rect::from_center_size(pos2(x, track_y), vec2(10.0, 24.0));
        let response = ui
            .interact(
                handle.expand(4.0),
                ui.scope_id().with(id),
                Sense::click_and_drag(),
            )
            .on_hover_cursor(CursorIcon::ResizeHorizontal);
        drag_frequency(setter, param, &response, |pointer_x| {
            let hz = hz_of(pointer_x);
            if is_low {
                hz.min(high / min_ratio)
            } else {
                hz.max(low * min_ratio)
            }
        });

        let fill = if response.hovered() || response.dragged() {
            GLOW
        } else {
            TEXT
        };
        painter.rect_filled(handle, 3, fill);
        painter.rect_stroke(handle, 3, Stroke::new(1.0, BACKGROUND), StrokeKind::Outside);
    }

    let small = FontId::proportional(11.0);
    let below = track_y + 22.0;
    painter.text(
        pos2(left, below),
        Align2::LEFT_CENTER,
        params.low_cut.to_string(),
        small.clone(),
        TEXT,
    );
    painter.text(
        pos2(left + width, below),
        Align2::RIGHT_CENTER,
        params.high_cut.to_string(),
        small.clone(),
        TEXT,
    );
    painter.text(
        pos2(left, rect.top()),
        Align2::LEFT_CENTER,
        "LOW CUT",
        small.clone(),
        DIM_TEXT,
    );
    painter.text(
        pos2(left + width, rect.top()),
        Align2::RIGHT_CENTER,
        "HIGH CUT",
        small,
        DIM_TEXT,
    );
    caption(painter, pos2(rect.center().x, below + 18.0), "BAND");
}

/// Handles a drag gesture on a frequency handle, mapping the pointer's x position through
/// `hz_at`. Double-click resets to the default.
fn drag_frequency(
    setter: &ParamSetter,
    param: &FloatParam,
    response: &egui::Response,
    hz_at: impl Fn(f32) -> f32,
) {
    if response.drag_started() {
        setter.begin_set_parameter(param);
    }
    if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        setter.set_parameter(param, hz_at(pointer.x));
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
    if response.double_clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter_normalized(param, param.default_normalized_value());
        setter.end_set_parameter(param);
    }
}

fn caption(painter: &Painter, center: Pos2, text: &str) {
    painter.text(
        center,
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(11.0),
        DIM_TEXT,
    );
}

/// Points along a circular arc, clockwise on screen from `start` over `sweep` radians.
fn arc_points(center: Pos2, radius: f32, start: f32, sweep: f32, segments: usize) -> Vec<Pos2> {
    (0..=segments)
        .map(|i| center + radius * Vec2::angled(start + sweep * i as f32 / segments as f32))
        .collect()
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)) as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// Sets a parameter as a single host-visible gesture.
fn set<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}
