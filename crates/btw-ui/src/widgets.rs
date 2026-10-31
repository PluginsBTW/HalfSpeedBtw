//! The PluginsBTW controls, drawn with egui's painter at absolute positions.
//!
//! Each control that changes a parameter reports it to the host as one gesture: begin, set
//! and end for a click, or begin on drag start, set on each move and end on drag stop.

use std::f32::consts::{PI, TAU};

use egui::{
    Align2, Color32, CursorIcon, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};
use nice_plug::prelude::{BoolParam, Enum, EnumParam, FloatParam, Param, ParamSetter};

use crate::geometry::{KNOB_START, KNOB_SWEEP, LogScale, arc_points, lerp_color};
use crate::palette::{self, *};
use crate::type_scale;

/// Window edge to panel edge.
pub const INSET_PANEL: f32 = 12.0;
/// Panel edge to content.
pub const PAD_CONTENT: f32 = 20.0;
/// Normalized change per pixel of knob drag; Shift drags ten times finer.
pub const DRAG_SPEED: f32 = 0.005;

/// Sets a parameter as a single host-visible gesture.
pub fn set_param<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

fn reset_param<P: Param>(setter: &ParamSetter, param: &P) {
    setter.begin_set_parameter(param);
    setter.set_parameter_normalized(param, param.default_normalized_value());
    setter.end_set_parameter(param);
}

/// Paints the window ground and the one panel on it, and returns the panel's rectangle.
pub fn panel(ui: &Ui) -> Rect {
    let window = ui.max_rect();
    let panel = window.shrink(INSET_PANEL);
    let painter = ui.painter();
    painter.rect_filled(window, 0, BACKGROUND);
    painter.rect_filled(panel, 14, SURFACE);
    painter.rect_stroke(panel, 14, Stroke::new(1.0, EDGE), StrokeKind::Inside);
    panel
}

/// An uppercase control name, centered on `center`.
pub fn caption(painter: &Painter, center: Pos2, text: &str) {
    painter.text(
        center,
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(type_scale::CAPTION),
        TEXT_DIM,
    );
}

/// A status line in the header, right-aligned to `right_center`.
pub fn status(painter: &Painter, right_center: Pos2, text: &str) {
    painter.text(
        right_center,
        Align2::RIGHT_CENTER,
        text,
        FontId::proportional(type_scale::CAPTION),
        TEXT_DIM,
    );
}

/// The clock mark and the product name: `first` in text, `second` in accent, then `Btw` in
/// dim text. Pass an empty `second` for one-word names. Returns the area it covers, for a
/// [`link`].
pub fn wordmark(painter: &Painter, mark_center: Pos2, first: &str, second: &str) -> Rect {
    // A slowed-down clock: a ring with one long, lazy hand.
    painter.circle_stroke(mark_center, 11.0, Stroke::new(2.5, ACCENT));
    let hand = Stroke::new(2.5, GLOW);
    painter.line_segment([mark_center, mark_center + vec2(0.0, -7.0)], hand);
    painter.line_segment([mark_center, mark_center + vec2(5.0, 3.0)], hand);

    let font = FontId::proportional(type_scale::WORDMARK);
    let mut area = Rect::from_center_size(mark_center, Vec2::splat(26.0));
    let mut x = mark_center.x + 20.0;
    for (word, color) in [(first, TEXT), (second, ACCENT), ("Btw", TEXT_DIM)] {
        let rect = painter.text(
            pos2(x, mark_center.y),
            Align2::LEFT_CENTER,
            word,
            font.clone(),
            color,
        );
        area = area.union(rect);
        x = rect.right();
    }
    area
}

/// Makes `rect` open `url` in the browser on click, with a pointing-hand cursor and `url` as
/// the tooltip. Draws nothing, so the wordmark keeps its look; use it on the wordmark's area.
pub fn link(ui: &Ui, rect: Rect, url: &str) {
    let response = ui
        .interact(rect, ui.scope_id().with(url), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(url);
    if response.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
}

/// A rounded on/off pill reading `<stem> ON` or `<stem> OFF`, with a tooltip saying what
/// "on" does. The design size is 92 x 26.
pub fn toggle_pill(
    ui: &Ui,
    setter: &ParamSetter,
    rect: Rect,
    param: &BoolParam,
    stem: &str,
    tooltip: &str,
) {
    let on = param.value();
    let response = ui
        .interact(rect, ui.scope_id().with(stem), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tooltip);
    if response.clicked() {
        set_param(setter, param, !on);
    }

    let (fill, stroke, text) = if on {
        (palette::accent_halo(), ACCENT, GLOW)
    } else {
        (RAISED, EDGE, TEXT_DIM)
    };
    let radius = (rect.height() / 2.0) as u8;
    let painter = ui.painter();
    painter.rect_filled(rect, radius, fill);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, stroke), StrokeKind::Inside);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{stem} {}", if on { "ON" } else { "OFF" }),
        FontId::proportional(type_scale::CAPTION),
        text,
    );
}

/// A row of equally sized buttons, one per enum variant, filling `rect`. Use 34px height
/// for a full-width row and 32px for a compact one; put a caption 14px below.
pub fn segmented<T: Enum + PartialEq + 'static>(
    ui: &Ui,
    setter: &ParamSetter,
    id_salt: &str,
    rect: Rect,
    param: &EnumParam<T>,
) {
    let names = T::variants();
    let gap = 6.0;
    let width = (rect.width() - gap * (names.len() - 1) as f32) / names.len() as f32;
    let current = param.value().to_index();
    let painter = ui.painter();

    for (index, name) in names.iter().enumerate() {
        let button = Rect::from_min_size(
            rect.min + vec2(index as f32 * (width + gap), 0.0),
            vec2(width, rect.height()),
        );
        let response = ui
            .interact(
                button,
                ui.scope_id().with(id_salt).with(index),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let selected = index == current;
        if response.clicked() && !selected {
            set_param(setter, param, T::from_index(index));
        }

        if selected {
            painter.rect_filled(button.expand(3.0), 10, palette::accent_halo_strong());
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
            (false, false) => TEXT_DIM,
        };
        painter.text(
            button.center(),
            Align2::CENTER_CENTER,
            *name,
            FontId::proportional(type_scale::CONTROL_LABEL),
            text,
        );
    }
}

/// Knob sizes, by arc radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnobSize {
    /// 38px: the primary pair of controls.
    Large,
    /// 32px: secondary controls.
    Small,
}

impl KnobSize {
    pub fn radius(self) -> f32 {
        match self {
            Self::Large => 38.0,
            Self::Small => 32.0,
        }
    }

    fn value_font(self) -> f32 {
        match self {
            Self::Large => type_scale::VALUE_LARGE,
            Self::Small => type_scale::VALUE,
        }
    }
}

/// A ring knob showing its value in the middle and `label` as a caption below. Drag up or
/// right to increase (Shift for fine control), double-click to reset.
pub fn knob<P: Param>(
    ui: &Ui,
    setter: &ParamSetter,
    center: Pos2,
    size: KnobSize,
    param: &P,
    label: &str,
) {
    let radius = size.radius();
    let id = ui.scope_id().with(label);
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
        reset_param(setter, param);
    }

    let value = param.unmodulated_normalized_value();
    let painter = ui.painter();
    painter.add(Shape::line(
        arc_points(center, radius, KNOB_START, KNOB_SWEEP, 48),
        Stroke::new(5.0, RAISED_HOVER),
    ));
    if value > 0.0 {
        let arc = arc_points(center, radius, KNOB_START, KNOB_SWEEP * value, 48);
        painter.add(Shape::line(
            arc.clone(),
            Stroke::new(9.0, palette::accent_halo()),
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
        FontId::proportional(size.value_font()),
        TEXT,
    );
    caption(painter, center + vec2(0.0, radius + 16.0), label);
}

/// The display: an inset screen with a beat grid, mirrored peak bars and a playhead.
///
/// `levels` are peak levels in `0..=1` across one cycle (any count; HalfSpeedBtw uses 256),
/// `phase` is the playhead position in `0..=1`. While `enabled` is false everything inside
/// is dimmed. Draw the [`power_switch`] on top afterwards, unless the plugin uses the
/// [`compact_power_switch`] in the header.
pub fn display(painter: &Painter, rect: Rect, levels: &[f32], phase: f32, enabled: bool) {
    let shade = |color: Color32| {
        if enabled {
            color
        } else {
            color.gamma_multiply(OPACITY_OFF)
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
            Stroke::new(1.0, palette::edge_faint()),
        );
    }

    if !levels.is_empty() {
        let bins = levels.len() as f32;
        let bin_width = inner.width() / bins;
        for (i, peak) in levels.iter().enumerate() {
            // Square root lifts quiet material so the shape stays readable.
            let level = peak.max(0.0).sqrt().min(1.0);
            let half_height = (level * inner.height() * 0.5).max(0.5);
            let x = inner.left() + i as f32 * bin_width;
            let played = (i as f32 + 0.5) / bins <= phase;
            let color = if played {
                lerp_color(ACCENT, GLOW, level)
            } else {
                palette::accent_dim()
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
    }

    let playhead_x = inner.left() + phase.clamp(0.0, 1.0) * inner.width();
    let playhead = [
        pos2(playhead_x, rect.top() + 6.0),
        pos2(playhead_x, rect.bottom() - 6.0),
    ];
    painter.line_segment(playhead, Stroke::new(5.0, shade(palette::glow_halo())));
    painter.line_segment(playhead, Stroke::new(1.5, shade(GLOW)));
}

/// The build's version, `v` plus `version`, as a dim caption right-aligned to
/// `right_center`: the panel's bottom-right corner (x 652, on the bottom row's caption line).
pub fn version_label(painter: &Painter, right_center: Pos2, version: &str) {
    painter.text(
        right_center,
        Align2::RIGHT_CENTER,
        format!("v{version}"),
        FontId::proportional(type_scale::CAPTION),
        TEXT_DIM,
    );
}

/// The power switch at header size: a 26px round button styled like the toggle pills next to
/// it, with the power symbol at half the size of the display-centered [`power_switch`]. Put it
/// at the content's right edge (center x 635, y 38), right of the pills. Use one of the two
/// switches per plugin, never both.
pub fn compact_power_switch(ui: &Ui, setter: &ParamSetter, center: Pos2, param: &BoolParam) {
    let on = param.value();
    let radius = 13.0;
    let response = ui
        .interact(
            Rect::from_center_size(center, Vec2::splat(2.0 * radius)),
            ui.scope_id().with("power"),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text("Turn the effect on or off");
    if response.clicked() {
        set_param(setter, param, !on);
    }

    let (fill, border, icon) = match (on, response.hovered()) {
        (true, _) => (palette::accent_halo(), ACCENT, GLOW),
        (false, true) => (RAISED, EDGE, TEXT),
        (false, false) => (RAISED, EDGE, TEXT_DIM),
    };
    let painter = ui.painter();
    painter.circle_filled(center, radius, fill);
    painter.circle_stroke(center, radius - 0.5, Stroke::new(1.0, border));
    let gap = 0.7;
    let arc = arc_points(center, 6.0, -PI / 2.0 + gap, TAU - 2.0 * gap, 24);
    painter.add(Shape::line(arc, Stroke::new(2.0, icon)));
    painter.line_segment(
        [center + vec2(0.0, -8.0), center + vec2(0.0, -2.0)],
        Stroke::new(2.0, icon),
    );
}

/// The round on/off switch for the whole effect, usually centered on the display.
pub fn power_switch(ui: &Ui, setter: &ParamSetter, center: Pos2, param: &BoolParam) {
    let on = param.value();
    let radius = 30.0;
    let response = ui
        .interact(
            Rect::from_center_size(center, Vec2::splat(2.0 * radius)),
            ui.scope_id().with("power"),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    if response.clicked() {
        set_param(setter, param, !on);
    }

    let ring = match (on, response.hovered()) {
        (true, _) => GLOW,
        (false, true) => TEXT,
        (false, false) => TEXT_DIM,
    };
    let painter = ui.painter();
    if on {
        painter.circle_filled(center, radius + 6.0, palette::accent_halo());
    }
    painter.circle_filled(center, radius, palette::scrim());
    painter.circle_stroke(center, radius, Stroke::new(2.0, ring));
    let gap = 0.7;
    let icon = arc_points(center, 12.0, -PI / 2.0 + gap, TAU - 2.0 * gap, 32);
    painter.add(Shape::line(icon, Stroke::new(3.0, ring)));
    painter.line_segment(
        [center + vec2(0.0, -16.0), center + vec2(0.0, -4.0)],
        Stroke::new(3.0, ring),
    );
}

/// Labels for a [`range_slider`]: the low and high ends' names and the group caption.
#[derive(Debug, Clone, Copy)]
pub struct RangeLabels<'a> {
    pub low: &'a str,
    pub high: &'a str,
    pub caption: &'a str,
}

/// A two-handled range slider on a logarithmic `scale`, for a low and a high bound that
/// must stay ordered. Handles stop 1.25x apart; double-click a handle to reset it.
pub fn range_slider(
    ui: &Ui,
    setter: &ParamSetter,
    rect: Rect,
    scale: LogScale,
    (low, high): (&FloatParam, &FloatParam),
    labels: RangeLabels,
) {
    let track_y = rect.top() + 18.0;
    let left = rect.left() + 6.0;
    let width = rect.width() - 12.0;
    let x_of = |value: f32| left + scale.fraction_of(value) * width;
    let value_at = |x: f32| scale.value_at((x - left) / width);

    let low_value = low.value();
    let high_value = high.value();
    let painter = ui.painter();
    let track = Rect::from_min_max(pos2(left, track_y - 3.0), pos2(left + width, track_y + 3.0));
    painter.rect_filled(track, 3, RAISED_HOVER);
    let active = Rect::from_min_max(
        pos2(x_of(low_value), track.top()),
        pos2(x_of(high_value), track.bottom()),
    );
    painter.rect_filled(active.expand(3.0), 6, palette::accent_halo());
    painter.rect_filled(active, 3, ACCENT);

    // Keep the handles from crossing so the range never closes completely.
    let min_ratio = 1.25;
    for (param, is_low) in [(low, true), (high, false)] {
        let x = x_of(param.value());
        let handle = Rect::from_center_size(pos2(x, track_y), vec2(10.0, 24.0));
        let response = ui
            .interact(
                handle.expand(4.0),
                ui.scope_id().with(labels.caption).with(is_low),
                Sense::click_and_drag(),
            )
            .on_hover_cursor(CursorIcon::ResizeHorizontal);
        drag_handle(setter, param, &response, |pointer_x| {
            let value = value_at(pointer_x);
            if is_low {
                value.min(high_value / min_ratio)
            } else {
                value.max(low_value * min_ratio)
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

    let small = FontId::proportional(type_scale::CAPTION);
    let below = track_y + 22.0;
    for (pos, align, text, color) in [
        (
            pos2(left, below),
            Align2::LEFT_CENTER,
            low.to_string(),
            TEXT,
        ),
        (
            pos2(left + width, below),
            Align2::RIGHT_CENTER,
            high.to_string(),
            TEXT,
        ),
        (
            pos2(left, rect.top()),
            Align2::LEFT_CENTER,
            labels.low.to_string(),
            TEXT_DIM,
        ),
        (
            pos2(left + width, rect.top()),
            Align2::RIGHT_CENTER,
            labels.high.to_string(),
            TEXT_DIM,
        ),
    ] {
        painter.text(pos, align, text, small.clone(), color);
    }
    caption(painter, pos2(rect.center().x, below + 18.0), labels.caption);
}

/// Handles a drag gesture on a slider handle, mapping the pointer's x position through
/// `value_at`. Double-click resets to the default.
fn drag_handle(
    setter: &ParamSetter,
    param: &FloatParam,
    response: &Response,
    value_at: impl Fn(f32) -> f32,
) {
    if response.drag_started() {
        setter.begin_set_parameter(param);
    }
    if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        setter.set_parameter(param, value_at(pointer.x));
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
    if response.double_clicked() {
        reset_param(setter, param);
    }
}

/// Lays out a strip of `N` related [`dropdown`]s: paints the strip (`raised`, hairline,
/// 30px tall in the design) and returns the area of each dropdown, 12px in from the ends and
/// 16px apart, sharing the width equally.
pub fn dropdown_strip<const N: usize>(painter: &Painter, rect: Rect) -> [Rect; N] {
    painter.rect_filled(rect, 7, RAISED);
    painter.rect_stroke(rect, 7, Stroke::new(1.0, EDGE), StrokeKind::Inside);
    let inner = rect.shrink2(vec2(12.0, 0.0));
    let gap = 16.0;
    let width = (inner.width() - gap * (N as f32 - 1.0)) / N as f32;
    std::array::from_fn(|i| {
        Rect::from_min_size(
            inner.min + vec2(i as f32 * (width + gap), 0.0),
            vec2(width, inner.height()),
        )
    })
}

/// Items per menu column; longer menus wrap into more columns so they stay inside the window.
pub const MENU_ROWS: usize = 7;
/// Width of one menu column.
pub const MENU_COLUMN_WIDTH: f32 = 140.0;
/// Height of one menu item.
pub const MENU_ITEM_HEIGHT: f32 = 24.0;

/// A dropdown for an enum parameter inside `area` (from [`dropdown_strip`]): the caption, the
/// value in accent and a chevron at the right end. A click opens a menu of every variant 6px
/// below the strip; choosing one closes it and sets the parameter as one gesture.
pub fn dropdown<T: Enum + PartialEq + 'static>(
    ui: &Ui,
    setter: &ParamSetter,
    area: Rect,
    caption: &str,
    param: &EnumParam<T>,
) {
    let current = param.value().to_index();
    if let Some(index) = dropdown_menu(ui, area, caption, T::variants(), current) {
        set_param(setter, param, T::from_index(index));
    }
}

/// The parameter-free part of [`dropdown`]: draws it, runs its menu and returns the index the
/// person chose, if it differs from `selected`.
pub fn dropdown_menu(
    ui: &Ui,
    area: Rect,
    caption: &str,
    options: &[&str],
    selected: usize,
) -> Option<usize> {
    let id = ui.scope_id().with("dropdown").with(caption);
    // One menu open at a time, across all dropdowns of the editor.
    let open_id = egui::Id::unique("btw_ui_open_dropdown");
    let is_open = ui.data(|data| data.get_temp::<egui::Id>(open_id)) == Some(id);

    let painter = ui.painter();
    let center_y = area.center().y;
    let caption_rect = painter.text(
        pos2(area.left(), center_y),
        Align2::LEFT_CENTER,
        caption,
        FontId::proportional(type_scale::CAPTION),
        TEXT_DIM,
    );
    let value_left = caption_rect.right() + 12.0;
    painter.text(
        pos2(value_left, center_y),
        Align2::LEFT_CENTER,
        options.get(selected).copied().unwrap_or_default(),
        FontId::proportional(type_scale::CONTROL_LABEL),
        ACCENT,
    );
    let chevron = pos2(area.right() - 4.0, center_y);
    painter.add(Shape::line(
        vec![
            chevron + vec2(-4.0, -2.0),
            chevron + vec2(0.0, 2.0),
            chevron + vec2(4.0, -2.0),
        ],
        Stroke::new(1.5, TEXT_DIM),
    ));

    let response = ui
        .interact(area, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    if response.clicked() {
        let next = if is_open { None } else { Some(id) };
        ui.data_mut(|data| match next {
            Some(id) => {
                data.insert_temp(open_id, id);
            }
            None => {
                data.remove::<egui::Id>(open_id);
            }
        });
        return None;
    }
    if !is_open {
        return None;
    }

    // The strip is 30px tall in the design; the menu hangs 6px below it.
    let strip_bottom = center_y + 15.0;
    let columns = options.len().div_ceil(MENU_ROWS).max(1);
    let rows = options.len().min(MENU_ROWS);
    let menu = Rect::from_min_size(
        pos2(value_left, strip_bottom + 6.0),
        vec2(
            columns as f32 * MENU_COLUMN_WIDTH,
            8.0 + rows as f32 * MENU_ITEM_HEIGHT,
        ),
    );

    let mut chosen = None;
    egui::Area::new(id.with("menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(menu.min)
        .constrain(false)
        .show(ui.ctx(), |menu_ui| {
            // Claim the whole menu, so the area has its size and nothing under it gets the
            // pointer.
            menu_ui.allocate_rect(menu, Sense::click());
            let painter = menu_ui.painter();
            painter.rect_filled(menu, 7, RAISED);
            painter.rect_stroke(menu, 7, Stroke::new(1.0, EDGE), StrokeKind::Inside);
            for (index, name) in options.iter().enumerate() {
                let item = Rect::from_min_size(
                    menu.min
                        + vec2(
                            (index / MENU_ROWS) as f32 * MENU_COLUMN_WIDTH,
                            4.0 + (index % MENU_ROWS) as f32 * MENU_ITEM_HEIGHT,
                        ),
                    vec2(MENU_COLUMN_WIDTH, MENU_ITEM_HEIGHT),
                );
                let item_response = menu_ui
                    .interact(item, id.with(index), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand);
                if item_response.hovered() {
                    painter.rect_filled(item, 0, RAISED_HOVER);
                }
                painter.text(
                    pos2(item.left() + 12.0, item.center().y),
                    Align2::LEFT_CENTER,
                    *name,
                    FontId::proportional(type_scale::CONTROL_LABEL),
                    if index == selected { ACCENT } else { TEXT },
                );
                if item_response.clicked() {
                    chosen = Some(index);
                }
            }
        });

    let clicked_elsewhere = ui.input(|input| {
        input.pointer.any_click()
            && input
                .pointer
                .interact_pos()
                .is_some_and(|pos| !menu.contains(pos) && !area.contains(pos))
    });
    let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
    if chosen.is_some() || clicked_elsewhere || escape {
        ui.data_mut(|data| data.remove::<egui::Id>(open_id));
    }
    chosen.filter(|&index| index != selected)
}

/// Whether a NoteKeyboard click removes notes (VocalTuneBtw) or picks the root (EasyBtw).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardMode {
    /// Clicking a note in the scale toggles it out of the scale and back. Keys outside the
    /// scale do nothing.
    Remove,
    /// Clicking any key makes it the root (the target). Clicking the root does nothing.
    RootPick,
}

/// What a [`note_keyboard`] shows, as 12-bit pitch-class masks (bit 0 = C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyboardKeys {
    /// Notes the scale allows.
    pub scale: u16,
    /// Scale notes the person removed: drawn dark with a hairline.
    pub removed: u16,
    /// The current target (the sung note, or the root), if any.
    pub target: Option<u8>,
    /// Notes sounding right now: `glow`, over every other state.
    pub held: u16,
}

/// Names on the keys; black keys carry both spellings on two lines.
const KEY_LABELS: [&str; 12] = [
    "C", "C#\nDb", "D", "D#\nEb", "E", "F", "F#\nGb", "G", "G#\nAb", "A", "A#\nBb", "B",
];
const BLACK_KEYS: [bool; 12] = [
    false, true, false, true, false, false, true, false, true, false, true, false,
];

/// One octave of twelve equal keys in a `display` frame filling `rect`, showing the scale,
/// the target and the held notes. While `enabled` is false everything, frame included, drops
/// to `opacity-off` and still takes clicks. Returns the pitch class a click asks for: a note
/// to toggle in [`KeyboardMode::Remove`], the new root in [`KeyboardMode::RootPick`].
pub fn note_keyboard(
    ui: &Ui,
    rect: Rect,
    keys: &KeyboardKeys,
    mode: KeyboardMode,
    enabled: bool,
) -> Option<u8> {
    let shade = |color: Color32| {
        if enabled {
            color
        } else {
            color.gamma_multiply(OPACITY_OFF)
        }
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 8, shade(DISPLAY));
    painter.rect_stroke(rect, 8, Stroke::new(1.0, shade(EDGE)), StrokeKind::Inside);

    let mut clicked = None;
    for (pc, area) in key_areas(rect).into_iter().enumerate() {
        let bit = 1u16 << pc;
        let black = BLACK_KEYS[pc];
        let in_scale = keys.scale & bit != 0;
        let removed = in_scale && keys.removed & bit != 0;
        let clickable = match mode {
            KeyboardMode::Remove => in_scale,
            KeyboardMode::RootPick => true,
        };

        // The column, gaps included, takes the clicks, so there are no dead strips.
        let column = Rect::from_x_y_ranges(area.x_range(), rect.y_range()).expand2(vec2(2.0, 0.0));
        let response = ui.interact(
            column,
            ui.scope_id().with("note_key").with(pc),
            if clickable {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let response = if clickable {
            response.on_hover_cursor(CursorIcon::PointingHand)
        } else {
            response
        };
        let is_target = keys.target == Some(pc as u8);
        if response.clicked() && !(mode == KeyboardMode::RootPick && is_target) {
            clicked = Some(pc as u8);
        }

        let (fill, border, halo, label) = if keys.held & bit != 0 {
            (GLOW, None, Some(palette::glow_halo()), BACKGROUND)
        } else if is_target {
            (
                ACCENT,
                None,
                Some(palette::accent_halo_strong()),
                BACKGROUND,
            )
        } else if in_scale && !removed {
            if black {
                (KEY_BLACK, None, None, TEXT)
            } else {
                (TEXT, None, None, BACKGROUND)
            }
        } else if removed {
            (DISPLAY, Some(EDGE), None, TEXT_DIM)
        } else {
            (DISPLAY, None, None, TEXT_DIM.gamma_multiply(0.35))
        };

        if let Some(halo) = halo {
            painter.rect_filled(area.expand(3.0), 10, shade(halo));
        }
        painter.rect_filled(area, 7, shade(fill));
        if let Some(border) = border {
            painter.rect_stroke(area, 7, Stroke::new(1.0, shade(border)), StrokeKind::Inside);
        }
        painter.text(
            pos2(area.center().x, area.bottom() - 8.0),
            Align2::CENTER_BOTTOM,
            KEY_LABELS[pc],
            FontId::proportional(type_scale::CONTROL_LABEL),
            shade(label),
        );
    }
    clicked
}

/// The twelve key rectangles inside a keyboard frame: 6px in from the frame, 4px apart,
/// black keys 10px taller than white keys at the top.
pub fn key_areas(rect: Rect) -> [Rect; 12] {
    let inner = rect.shrink(6.0);
    let gap = 4.0;
    let width = (inner.width() - 11.0 * gap) / 12.0;
    std::array::from_fn(|pc| {
        let top = if BLACK_KEYS[pc] {
            inner.top()
        } else {
            inner.top() + 10.0
        };
        Rect::from_min_max(
            pos2(inner.left() + pc as f32 * (width + gap), top),
            pos2(
                inner.left() + pc as f32 * (width + gap) + width,
                inner.bottom(),
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://example.com/plugin";

    /// Runs one frame with `events`, placing a link over a 100 x 30 area at the top left.
    fn frame(ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            link(ui, Rect::from_min_size(Pos2::ZERO, vec2(100.0, 30.0)), URL);
        });
        // No renderer here to take the font atlas uploads.
        output.textures_delta.clear();
        output
    }

    fn pointer(button: egui::PointerButton, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: pos2(50.0, 15.0),
            button,
            pressed,
            modifiers: Default::default(),
        }
    }

    fn opened_urls(output: &egui::FullOutput) -> Vec<String> {
        output
            .platform_output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn link_shows_a_pointing_hand_and_opens_its_url_on_click() {
        let ctx = egui::Context::default();
        frame(&ctx, vec![]);
        let hover = frame(&ctx, vec![egui::Event::PointerMoved(pos2(50.0, 15.0))]);
        assert_eq!(hover.platform_output.cursor_icon, CursorIcon::PointingHand);
        assert!(opened_urls(&hover).is_empty());

        frame(&ctx, vec![pointer(egui::PointerButton::Primary, true)]);
        let click = frame(&ctx, vec![pointer(egui::PointerButton::Primary, false)]);
        assert_eq!(opened_urls(&click), [URL]);
    }

    #[test]
    fn link_ignores_the_pointer_outside_its_area() {
        let ctx = egui::Context::default();
        frame(&ctx, vec![]);
        let away = frame(&ctx, vec![egui::Event::PointerMoved(pos2(150.0, 80.0))]);
        assert_eq!(away.platform_output.cursor_icon, CursorIcon::Default);
    }

    /// Runs one frame of a 320 x 260 keyboard with `events` and returns what it reported.
    fn keyboard_frame(
        ctx: &egui::Context,
        keys: &KeyboardKeys,
        mode: KeyboardMode,
        events: Vec<egui::Event>,
    ) -> (Option<u8>, egui::FullOutput) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..Default::default()
        };
        let mut clicked = None;
        let mut output = ctx.run_ui(input, |ui| {
            let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(320.0, 260.0));
            clicked = note_keyboard(ui, rect, keys, mode, true);
        });
        output.textures_delta.clear();
        (clicked, output)
    }

    fn click_at(pos: Pos2) -> [Vec<egui::Event>; 2] {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        [
            vec![egui::Event::PointerMoved(pos), button(true)],
            vec![button(false)],
        ]
    }

    /// Clicks the middle of key `pc` and returns what the keyboard reported.
    fn click_key(keys: &KeyboardKeys, mode: KeyboardMode, pc: usize) -> Option<u8> {
        let ctx = egui::Context::default();
        keyboard_frame(&ctx, keys, mode, vec![]);
        let center = key_areas(Rect::from_min_size(Pos2::ZERO, vec2(320.0, 260.0)))[pc].center();
        let [press, release] = click_at(center);
        keyboard_frame(&ctx, keys, mode, press);
        keyboard_frame(&ctx, keys, mode, release).0
    }

    const F_MINOR: KeyboardKeys = KeyboardKeys {
        scale: 0b0101_1010_1011,
        removed: 0,
        target: Some(5),
        held: 1 << 8,
    };

    #[test]
    fn root_pick_reports_any_key_but_the_current_root() {
        assert_eq!(click_key(&F_MINOR, KeyboardMode::RootPick, 7), Some(7));
        // E is outside F minor and can still become the root.
        assert_eq!(click_key(&F_MINOR, KeyboardMode::RootPick, 4), Some(4));
        assert_eq!(click_key(&F_MINOR, KeyboardMode::RootPick, 5), None);
    }

    #[test]
    fn remove_mode_ignores_keys_outside_the_scale() {
        assert_eq!(click_key(&F_MINOR, KeyboardMode::Remove, 7), Some(7));
        assert_eq!(click_key(&F_MINOR, KeyboardMode::Remove, 4), None);
    }

    #[test]
    fn keyboard_cursor_follows_the_mode() {
        let ctx = egui::Context::default();
        let e_key = key_areas(Rect::from_min_size(Pos2::ZERO, vec2(320.0, 260.0)))[4].center();
        for (mode, cursor) in [
            (KeyboardMode::RootPick, CursorIcon::PointingHand),
            (KeyboardMode::Remove, CursorIcon::Default),
        ] {
            keyboard_frame(&ctx, &F_MINOR, mode, vec![]);
            let (_, output) =
                keyboard_frame(&ctx, &F_MINOR, mode, vec![egui::Event::PointerMoved(e_key)]);
            assert_eq!(output.platform_output.cursor_icon, cursor, "{mode:?}");
        }
    }

    #[test]
    fn key_areas_share_the_width_and_lift_black_keys() {
        let areas = key_areas(Rect::from_min_size(pos2(32.0, 110.0), vec2(616.0, 230.0)));
        assert_eq!(areas[0].left(), 38.0);
        assert!((areas[11].right() - 642.0).abs() < 1e-3);
        assert_eq!(areas[1].top(), 116.0);
        assert_eq!(areas[0].top(), 126.0);
        assert!(areas.iter().all(|key| key.bottom() == 334.0));
        assert!((areas[1].left() - areas[0].right() - 4.0).abs() < 1e-3);
    }

    const OPTIONS: [&str; 9] = ["A", "B", "C", "D", "E", "F", "G", "H", "I"];

    /// Runs one frame of a dropdown over x 0-300, y 0-30 and returns what it chose.
    fn dropdown_frame(ctx: &egui::Context, events: Vec<egui::Event>) -> Option<usize> {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0))),
            events,
            ..Default::default()
        };
        let mut chosen = None;
        let mut output = ctx.run_ui(input, |ui| {
            let area = Rect::from_min_size(pos2(12.0, 0.0), vec2(276.0, 30.0));
            chosen = dropdown_menu(ui, area, "KEY", &OPTIONS, 2);
        });
        output.textures_delta.clear();
        chosen
    }

    /// Moves the pointer to `pos`, then clicks there, as a person would.
    fn click(ctx: &egui::Context, pos: Pos2) -> Option<usize> {
        let [press, release] = click_at(pos);
        dropdown_frame(ctx, vec![egui::Event::PointerMoved(pos)]);
        dropdown_frame(ctx, press);
        dropdown_frame(ctx, release)
    }

    fn menu_is_open(ctx: &egui::Context) -> bool {
        ctx.data(|data| data.get_temp::<egui::Id>(egui::Id::unique("btw_ui_open_dropdown")))
            .is_some()
    }

    #[test]
    fn dropdown_opens_chooses_and_closes() {
        let ctx = egui::Context::default();
        dropdown_frame(&ctx, vec![]);
        assert_eq!(click(&ctx, pos2(100.0, 15.0)), None);
        assert!(menu_is_open(&ctx));
        dropdown_frame(&ctx, vec![]);

        // Nine options: two columns, seven rows in the first. The menu starts 6px below the
        // strip at the value's left edge, about 44px in after the caption; 40px into the
        // second column is its first item ("H") whatever the caption's exact width.
        let menu_left = 44.0;
        let item = pos2(menu_left + MENU_COLUMN_WIDTH + 40.0, 36.0 + 4.0 + 12.0);
        assert_eq!(click(&ctx, item), Some(7));
        assert!(!menu_is_open(&ctx));
    }

    #[test]
    fn dropdown_closes_without_a_change() {
        let ctx = egui::Context::default();
        dropdown_frame(&ctx, vec![]);
        click(&ctx, pos2(100.0, 15.0));
        assert!(menu_is_open(&ctx));
        assert_eq!(click(&ctx, pos2(390.0, 390.0)), None);
        assert!(!menu_is_open(&ctx));

        click(&ctx, pos2(100.0, 15.0));
        assert!(menu_is_open(&ctx));
        let escape = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        assert_eq!(dropdown_frame(&ctx, vec![escape]), None);
        assert!(!menu_is_open(&ctx));

        // A second click on the dropdown closes it too.
        click(&ctx, pos2(100.0, 15.0));
        click(&ctx, pos2(100.0, 15.0));
        assert!(!menu_is_open(&ctx));
    }
}
