//! HalfTime plugin shell: wires the `halftime-dsp` engine into nice-plug's CLAP and VST3
//! wrappers. Keep DSP logic out of this crate so the engine stays testable without a host.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use atomic_float::AtomicF32;
use halftime_dsp::engine::DEFAULT_TEMPO_BPM;
use halftime_dsp::{HalfSpeedEngine, Timing};
use nice_plug::prelude::*;
use nice_plug_egui::{EguiEditor, EguiEditorState, EguiNiceSettings, RepaintNotifier};

mod editor;
mod params;

use editor::HalfTimeEditor;
pub use params::HalfTimeParams;

/// The longest lag the engine can replay. Covers 4 bars of 4/4 at 0.25x down to 40 BPM.
const MAX_LOOP_SECONDS: f32 = 20.0;

/// Values the audio thread publishes for the editor to display.
#[derive(Default)]
pub struct DisplayState {
    pub sample_rate: AtomicF32,
    /// Progress through the current loop cycle, `0..1`.
    pub cycle_phase: AtomicF32,
}

pub struct HalfTime {
    params: Arc<HalfTimeParams>,
    editor_state: Arc<EguiEditorState>,
    repaint_notifier: RepaintNotifier,
    display: Arc<DisplayState>,
    engine: HalfSpeedEngine,
}

impl Default for HalfTime {
    fn default() -> Self {
        Self {
            params: Arc::new(HalfTimeParams::default()),
            editor_state: EguiEditorState::from_size(editor::WINDOW_SIZE, 1.0),
            repaint_notifier: RepaintNotifier::new(),
            display: Arc::new(DisplayState::default()),
            // Resized in `activate()` once the real layout and sample rate are known.
            engine: HalfSpeedEngine::new(44_100.0, 0, 0.0),
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
            HalfTimeEditor::new(self.params.clone(), self.display.clone()),
        )
    }

    fn activate(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        let num_channels = audio_io_layout
            .main_output_channels
            .map_or(0, NonZeroU32::get) as usize;
        self.engine
            .prepare(buffer_config.sample_rate, num_channels, MAX_LOOP_SECONDS);
        self.display
            .sample_rate
            .store(buffer_config.sample_rate, Ordering::Relaxed);
        self.repaint_notifier.request_repaint();
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let transport = context.transport();
        let beats_per_bar = match (transport.time_sig_numerator, transport.time_sig_denominator) {
            (Some(num), Some(den)) if num > 0 && den > 0 => num as f64 * 4.0 / den as f64,
            _ => 4.0,
        };
        let timing = Timing {
            tempo_bpm: transport.tempo.unwrap_or(DEFAULT_TEMPO_BPM),
            beat: if self.params.sync.value() && transport.playing {
                transport.pos_beats()
            } else {
                None
            },
        };

        let settings = self.params.settings(beats_per_bar);
        self.engine.process(buffer.as_slice(), &settings, &timing);

        if self.editor_state.is_open() {
            self.display
                .cycle_phase
                .store(self.engine.cycle_phase(), Ordering::Relaxed);
            self.repaint_notifier.request_repaint();
        }

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
