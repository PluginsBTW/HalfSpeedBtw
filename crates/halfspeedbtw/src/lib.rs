//! HalfSpeed plugin shell: wires the `halfspeed-dsp` engine into nice-plug's CLAP and VST3
//! wrappers. Keep DSP logic out of this crate so the engine stays testable without a host.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use atomic_float::AtomicF32;
use halfspeed_dsp::engine::{DEFAULT_TEMPO_BPM, SCOPE_BINS};
use halfspeed_dsp::{HalfSpeedEngine, Timing};
use nice_plug::prelude::*;
use nice_plug_egui::{EguiEditor, EguiEditorState, EguiNiceSettings, RepaintNotifier};

mod editor;
mod params;

use editor::HalfSpeedEditor;
pub use params::HalfSpeedParams;

/// The longest lag the engine can replay. Covers 8 bars of 4/4 at 4x slowdown down to 36 BPM.
const MAX_LOOP_SECONDS: f32 = 40.0;

/// Values the audio thread publishes for the editor to display.
pub struct DisplayState {
    pub sample_rate: AtomicF32,
    /// Progress through the current loop cycle, `0..1`.
    pub cycle_phase: AtomicF32,
    /// Peak wet level per slice of the loop cycle, for the waveform display.
    pub scope: [AtomicF32; SCOPE_BINS],
}

impl Default for DisplayState {
    fn default() -> Self {
        Self {
            sample_rate: AtomicF32::new(0.0),
            cycle_phase: AtomicF32::new(0.0),
            scope: std::array::from_fn(|_| AtomicF32::new(0.0)),
        }
    }
}

pub struct HalfSpeed {
    params: Arc<HalfSpeedParams>,
    editor_state: Arc<EguiEditorState>,
    repaint_notifier: RepaintNotifier,
    display: Arc<DisplayState>,
    engine: HalfSpeedEngine,
}

impl Default for HalfSpeed {
    fn default() -> Self {
        Self {
            params: Arc::new(HalfSpeedParams::default()),
            editor_state: EguiEditorState::from_size(editor::WINDOW_SIZE, 1.0),
            repaint_notifier: RepaintNotifier::new(),
            display: Arc::new(DisplayState::default()),
            // Resized in `activate()` once the real layout and sample rate are known.
            engine: HalfSpeedEngine::new(44_100.0, 0, 0.0),
        }
    }
}

impl Plugin for HalfSpeed {
    const NAME: &'static str = "HalfSpeedBtw";
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

    type Editor = EguiEditor<HalfSpeedEditor>;
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
            HalfSpeedEditor::new(self.params.clone(), self.display.clone()),
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
        let tempo_bpm = transport
            .tempo
            .filter(|t| *t > 0.0)
            .unwrap_or(DEFAULT_TEMPO_BPM);
        let timing = Timing {
            tempo_bpm,
            beat: if self.params.sync.value() && transport.playing {
                transport.pos_beats()
            } else {
                None
            },
        };

        let settings = self.params.settings(beats_per_bar, tempo_bpm);
        self.engine.process(buffer.as_slice(), &settings, &timing);

        if self.editor_state.is_open() {
            self.display
                .cycle_phase
                .store(self.engine.cycle_phase(), Ordering::Relaxed);
            for (slot, peak) in self.display.scope.iter().zip(self.engine.scope()) {
                slot.store(*peak, Ordering::Relaxed);
            }
            self.repaint_notifier.request_repaint();
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for HalfSpeed {
    const CLAP_ID: &'static str = "com.romanhofmann.halfspeedbtw";
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

impl Vst3Plugin for HalfSpeed {
    // Stable forever: hosts identify saved instances by this ID.
    const VST3_CLASS_ID: [u8; 16] = *b"HalfSpeedBtwRH01";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::PitchShift];
}

nice_export_clap!(HalfSpeed);
nice_export_vst3!(HalfSpeed);
