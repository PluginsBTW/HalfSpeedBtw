//! HalfTime plugin shell: wires the `halftime-dsp` engine into nice-plug's CLAP and VST3
//! wrappers. Keep DSP logic out of this crate so the engine stays testable without a host.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use atomic_float::AtomicF32;
use nice_plug::prelude::*;
use nice_plug_egui::{EguiEditor, EguiEditorState, EguiNiceSettings, RepaintNotifier};

mod editor;
mod params;

use editor::HalfTimeEditor;
pub use params::HalfTimeParams;

pub struct HalfTime {
    params: Arc<HalfTimeParams>,
    editor_state: Arc<EguiEditorState>,
    repaint_notifier: RepaintNotifier,
    sample_rate: Arc<AtomicF32>,
}

impl Default for HalfTime {
    fn default() -> Self {
        Self {
            params: Arc::new(HalfTimeParams::default()),
            editor_state: EguiEditorState::from_size(editor::WINDOW_SIZE, 1.0),
            repaint_notifier: RepaintNotifier::new(),
            sample_rate: Arc::new(AtomicF32::new(0.0)),
        }
    }
}

impl Plugin for HalfTime {
    const NAME: &'static str = "HalfTime";
    const VENDOR: &'static str = "Roman Hofmann";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type Editor = EguiEditor<HalfTimeEditor>;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        nice_plug_egui::create_egui_editor(
            self.editor_state.clone(),
            self.repaint_notifier.clone(),
            EguiNiceSettings::new().with_resize_hint(ResizeHint::non_resizable()),
            HalfTimeEditor::new(self.params.clone(), self.sample_rate.clone()),
        )
    }

    fn activate(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        self.sample_rate
            .store(buffer_config.sample_rate, Ordering::Relaxed);
        self.repaint_notifier.request_repaint();
        true
    }

    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Pass-through: nice-plug processes in place, so leaving the buffer untouched forwards
        // the input unchanged with zero added latency.
        ProcessStatus::Normal
    }
}

impl ClapPlugin for HalfTime {
    const CLAP_ID: &'static str = "com.romanhofmann.halftime";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Tempo-synced half-speed loop effect");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Glitch,
    ];
}

impl Vst3Plugin for HalfTime {
    // Stable forever: hosts identify saved instances by this ID.
    const VST3_CLASS_ID: [u8; 16] = *b"HalfTimeRHofmann";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::PitchShift];
}

nice_export_clap!(HalfTime);
nice_export_vst3!(HalfTime);
