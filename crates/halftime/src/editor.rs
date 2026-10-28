//! Placeholder egui editor. Proves the GUI stack and parameter round-trips work; the reference
//! design (central control, waveform, loop buttons) comes in a later phase.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use atomic_float::AtomicF32;
use egui::{Align, Color32, CornerRadius, Layout, RichText, Vec2};
use nice_plug::{context::gui::GuiContext, editor::dpi::LogicalSize};
use nice_plug_egui::{NiceEguiApp, widgets};

use crate::params::HalfTimeParams;

pub const WINDOW_SIZE: LogicalSize<f32> = LogicalSize::new(360.0, 220.0);

const BACKGROUND: Color32 = Color32::from_rgb(0x12, 0x12, 0x16);
const ACCENT: Color32 = Color32::from_rgb(0xff, 0xb3, 0x2e);
const DIM_TEXT: Color32 = Color32::from_rgb(0x8a, 0x8a, 0x96);

pub struct HalfTimeEditor {
    params: Arc<HalfTimeParams>,
    /// Sample rate reported by the host in `initialize()`, shown so host setup is visible.
    sample_rate: Arc<AtomicF32>,
    gui_ctx: Option<GuiContext>,
}

impl HalfTimeEditor {
    pub fn new(params: Arc<HalfTimeParams>, sample_rate: Arc<AtomicF32>) -> Self {
        Self {
            params,
            sample_rate,
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

        egui::Frame::new()
            .fill(BACKGROUND)
            .inner_margin(16)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("HALFTIME").size(22.0).strong().color(ACCENT));
                    ui.add_space(12.0);

                    let enabled = self.params.enabled.value();
                    let (label, fill) = if enabled {
                        ("EFFECT ON", ACCENT)
                    } else {
                        ("EFFECT OFF", Color32::from_gray(0x30))
                    };
                    let text_color = if enabled { BACKGROUND } else { DIM_TEXT };
                    let button = egui::Button::new(RichText::new(label).strong().color(text_color))
                        .fill(fill)
                        .corner_radius(CornerRadius::same(6))
                        .min_size(Vec2::new(160.0, 36.0));
                    if ui.add(button).clicked() {
                        setter.begin_set_parameter(&self.params.enabled);
                        setter.set_parameter(&self.params.enabled, !enabled);
                        setter.end_set_parameter(&self.params.enabled);
                    }

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("MIX").color(DIM_TEXT));
                        ui.add(widgets::ParamSlider::for_param(&self.params.mix, &setter));
                    });
                });

                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    let sample_rate = self.sample_rate.load(Ordering::Relaxed);
                    ui.label(
                        RichText::new(format!(
                            "pass-through build · {:.1} kHz · 0 samples latency",
                            sample_rate / 1000.0
                        ))
                        .small()
                        .color(DIM_TEXT),
                    );
                });
            });
    }
}
