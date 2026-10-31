//! The HalfSpeedBtw editor: SYNC and the power switch in the header, loop buttons below it,
//! the waveform display in the middle flanked by fade knobs, mode buttons below it, and
//! smoothness, band and mix controls along the bottom, with the version in the corner.
//!
//! The look and the controls come from `btw-ui`; this file only lays them out on the fixed
//! 680 x 480 window and connects them to the parameters.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use btw_ui::geometry::LogScale;
use btw_ui::widgets::{self, KnobSize, RangeLabels};
use egui::{Rect, Ui, vec2};
use halfspeed_dsp::band::{MAX_HZ, MIN_HZ};
use halfspeed_dsp::engine::SCOPE_BINS;
use nice_plug::{context::gui::GuiContext, editor::dpi::LogicalSize};
use nice_plug_egui::NiceEguiApp;

use crate::DisplayState;
use crate::params::HalfSpeedParams;

pub const WINDOW_SIZE: LogicalSize<f32> = LogicalSize::new(680.0, 480.0);

pub struct HalfSpeedEditor {
    params: Arc<HalfSpeedParams>,
    display: Arc<DisplayState>,
    gui_ctx: Option<GuiContext>,
    /// The display's levels, copied out of the shared atomics once per frame.
    levels: [f32; SCOPE_BINS],
}

impl HalfSpeedEditor {
    pub fn new(params: Arc<HalfSpeedParams>, display: Arc<DisplayState>) -> Self {
        Self {
            params,
            display,
            gui_ctx: None,
            levels: [0.0; SCOPE_BINS],
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
        let origin = ui.max_rect().min;
        let at =
            |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(origin + vec2(x, y), vec2(w, h));
        let point = |x: f32, y: f32| origin + vec2(x, y);
        let painter = ui.painter().clone();

        widgets::panel(ui);

        // Header.
        widgets::wordmark(&painter, point(42.0, 38.0), "Half", "Speed");
        // Right to left from the panel's content edge: power, SYNC, status.
        let sample_rate = self.display.sample_rate.load(Ordering::Relaxed);
        widgets::status(
            &painter,
            point(508.0, 38.0),
            &format!("{:.1} kHz · 0 latency", sample_rate / 1000.0),
        );
        widgets::toggle_pill(
            ui,
            &setter,
            at(524.0, 25.0, 92.0, 26.0),
            &params.sync,
            "SYNC",
            "Follow the host's bar grid while playing",
        );
        widgets::compact_power_switch(ui, &setter, point(635.0, 38.0), &params.enabled);

        widgets::segmented(
            ui,
            &setter,
            "loop",
            at(32.0, 64.0, 616.0, 34.0),
            &params.loop_length,
        );
        widgets::caption(&painter, point(340.0, 112.0), "LOOP");

        // Center: fade knobs around the display.
        widgets::knob(
            ui,
            &setter,
            point(95.0, 205.0),
            KnobSize::Large,
            &params.fade_in,
            "FADE IN",
        );
        for (level, slot) in self.levels.iter_mut().zip(&self.display.scope) {
            *level = slot.load(Ordering::Relaxed);
        }
        let display = at(170.0, 128.0, 340.0, 166.0);
        widgets::display(
            &painter,
            display,
            &self.levels,
            self.display.cycle_phase.load(Ordering::Relaxed),
            params.enabled.value(),
        );
        widgets::knob(
            ui,
            &setter,
            point(585.0, 205.0),
            KnobSize::Large,
            &params.fade_out,
            "FADE OUT",
        );

        widgets::segmented(
            ui,
            &setter,
            "mode",
            at(230.0, 314.0, 220.0, 32.0),
            &params.ratio,
        );
        widgets::caption(&painter, point(340.0, 360.0), "MODE");

        // Bottom: smooth and mix around the band.
        widgets::knob(
            ui,
            &setter,
            point(95.0, 405.0),
            KnobSize::Small,
            &params.smoothness,
            "SMOOTH",
        );
        widgets::range_slider(
            ui,
            &setter,
            at(190.0, 386.0, 300.0, 50.0),
            LogScale::new(MIN_HZ, MAX_HZ),
            (&params.low_cut, &params.high_cut),
            RangeLabels {
                low: "LOW CUT",
                high: "HIGH CUT",
                caption: "BAND",
            },
        );
        widgets::knob(
            ui,
            &setter,
            point(585.0, 405.0),
            KnobSize::Small,
            &params.mix,
            "MIX",
        );

        widgets::version_label(&painter, point(652.0, 456.0), env!("CARGO_PKG_VERSION"));
    }
}
