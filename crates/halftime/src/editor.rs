//! egui editor: a central effect switch with the loop's progress around it, selectors for the
//! discrete parameters and sliders for the continuous ones. The waveform display comes later.

use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Vec2, pos2};
use nice_plug::prelude::{Enum, EnumParam, Param, ParamSetter};
use nice_plug::{context::gui::GuiContext, editor::dpi::LogicalSize};
use nice_plug_egui::{NiceEguiApp, widgets};

use crate::DisplayState;
use crate::params::HalfTimeParams;

pub const WINDOW_SIZE: LogicalSize<f32> = LogicalSize::new(520.0, 400.0);

const BACKGROUND: Color32 = Color32::from_rgb(0x12, 0x12, 0x16);
const PANEL: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x24);
const ACCENT: Color32 = Color32::from_rgb(0xff, 0xb3, 0x2e);
const DIM_TEXT: Color32 = Color32::from_rgb(0x8a, 0x8a, 0x96);

const DIAL_RADIUS: f32 = 64.0;
const RING_WIDTH: f32 = 6.0;
const LABEL_WIDTH: f32 = 90.0;

pub struct HalfTimeEditor {
    params: Arc<HalfTimeParams>,
    display: Arc<DisplayState>,
    gui_ctx: Option<GuiContext>,
}

impl HalfTimeEditor {
    pub fn new(params: Arc<HalfTimeParams>, display: Arc<DisplayState>) -> Self {
        Self {
            params,
            display,
            gui_ctx: None,
        }
    }
}

impl NiceEguiApp for HalfTimeEditor {
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

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut nice_plug_egui::Frame) {
        let Some(gui_ctx) = self.gui_ctx.as_ref() else {
            return;
        };
        let setter = gui_ctx.param_setter();
        let params = &self.params;

        egui::Frame::new()
            .fill(BACKGROUND)
            .inner_margin(16)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("HALFTIME").size(20.0).strong().color(ACCENT));
                    ui.add_space(8.0);
                    let phase = self.display.cycle_phase.load(Ordering::Relaxed);
                    effect_dial(ui, &setter, params, phase);
                    ui.add_space(12.0);
                });

                egui::Frame::new()
                    .fill(PANEL)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(12)
                    .show(ui, |ui| {
                        enum_row(ui, &setter, "SPEED", &params.ratio);
                        enum_row(ui, &setter, "LOOP", &params.loop_length);
                        enum_row(ui, &setter, "FADE", &params.fade);
                        labeled(ui, "SMOOTHNESS", |ui| {
                            ui.add(widgets::ParamSlider::for_param(&params.smoothness, &setter));
                        });
                        labeled(ui, "MIX", |ui| {
                            ui.add(widgets::ParamSlider::for_param(&params.mix, &setter));
                        });
                        labeled(ui, "SYNC", |ui| {
                            let sync = params.sync.value();
                            let text = if sync { "Host tempo" } else { "Free" };
                            if ui.selectable_label(sync, text).clicked() {
                                set(&setter, &params.sync, !sync);
                            }
                        });
                    });

                let sample_rate = self.display.sample_rate.load(Ordering::Relaxed);
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "{:.1} kHz · 0 samples latency",
                        sample_rate / 1000.0
                    ))
                    .small()
                    .color(DIM_TEXT),
                );
            });
    }
}

/// The round on/off switch in the middle, ringed by the loop's progress.
fn effect_dial(ui: &mut egui::Ui, setter: &ParamSetter, params: &HalfTimeParams, phase: f32) {
    let enabled = params.enabled.value();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::splat(2.0 * DIAL_RADIUS + RING_WIDTH), Sense::click());
    if response.clicked() {
        set(setter, &params.enabled, !enabled);
    }

    let painter = ui.painter_at(rect);
    let center = rect.center();
    let fill = if enabled { ACCENT } else { PANEL };
    painter.circle_filled(center, DIAL_RADIUS - RING_WIDTH, fill);
    painter.circle_stroke(center, DIAL_RADIUS, Stroke::new(RING_WIDTH, PANEL));

    // Progress arc, clockwise from 12 o'clock.
    let segments = (64.0 * phase).ceil() as usize;
    if segments > 0 {
        let points = (0..=segments)
            .map(|i| {
                let angle = TAU * phase * i as f32 / segments as f32 - TAU / 4.0;
                center + DIAL_RADIUS * Vec2::angled(angle)
            })
            .collect();
        painter.add(egui::Shape::line(points, Stroke::new(RING_WIDTH, ACCENT)));
    }

    let (label, color) = if enabled {
        ("ON", BACKGROUND)
    } else {
        ("OFF", DIM_TEXT)
    };
    painter.text(
        pos2(center.x, center.y),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(22.0),
        color,
    );
}

/// A row of buttons, one per enum variant, highlighting the current value.
fn enum_row<T: Enum + PartialEq + 'static>(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    label: &str,
    param: &EnumParam<T>,
) {
    labeled(ui, label, |ui| {
        let current = param.value().to_index();
        for (index, name) in T::variants().iter().enumerate() {
            if ui.selectable_label(index == current, *name).clicked() && index != current {
                set(setter, param, T::from_index(index));
            }
        }
    });
}

fn labeled(ui: &mut egui::Ui, label: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [LABEL_WIDTH, 20.0],
            egui::Label::new(RichText::new(label).small().color(DIM_TEXT)),
        );
        add_contents(ui);
    });
}

/// Sets a parameter as a single host-visible gesture.
fn set<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}
