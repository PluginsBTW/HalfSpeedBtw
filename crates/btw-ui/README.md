# btw-ui

The PluginsBTW UI design system as a Rust crate, for
[nice-plug](https://codeberg.org/RustAudio/nice-plug) plugin editors built with
egui: the palette, the type scale, and every control from HalfSpeedBtw.

## Use it in another plugin

Add it as a git dependency. Pin a commit or tag so a later change here can't
alter your plugin's look by surprise:

```toml
[dependencies]
btw-ui = { git = "https://github.com/PluginsBTW/HalfSpeedBtw", rev = "<commit>" }
nice-plug = "0.4.2"
nice-plug-egui = { version = "0.6.0", default-features = false, features = ["wgpu"] }
egui = { version = "0.37.0", features = ["default_fonts"] }
```

Your plugin must use the same major versions of nice-plug (0.4) and egui (0.37)
as `btw-ui`, so the widgets and your editor share one set of types. Render with
wgpu as above: the OpenGL renderer crashes on NVIDIA's proprietary driver.

Then lay the controls out in your `NiceEguiApp::ui`:

```rust
use btw_ui::widgets::{self, KnobSize};

fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut nice_plug_egui::Frame) {
    let Some(gui_ctx) = self.gui_ctx.as_ref() else { return };
    let setter = gui_ctx.param_setter();
    let painter = ui.painter().clone();
    let origin = ui.max_rect().min;

    widgets::panel(ui);
    widgets::wordmark(&painter, origin + egui::vec2(42.0, 38.0), "Tape", "Stop");
    widgets::knob(ui, &setter, origin + egui::vec2(95.0, 205.0), KnobSize::Large, &self.params.time, "TIME");
}
```

## What's in it

| Module | Contents |
| --- | --- |
| `palette` | The colors (`BACKGROUND`, `SURFACE`, `RAISED`, `RAISED_HOVER`, `EDGE`, `DISPLAY`, `ACCENT`, `GLOW`, `TEXT`, `TEXT_DIM`), the translucent variants (`accent_halo()` and friends), `OPACITY_OFF`, and a WCAG `contrast()` function |
| `type_scale` | The five font sizes: `WORDMARK` 22, `CONTROL_LABEL` and `VALUE_LARGE` 13, `VALUE` 12, `CAPTION` 11 |
| `widgets` | `panel`, `wordmark`, `status`, `caption`, `toggle_pill`, `segmented`, `knob`, `display`, `power_switch`, `range_slider`, and `set_param` for one-gesture parameter changes |
| `geometry` | `arc_points`, `lerp_color`, and `LogScale` for logarithmic sliders |

Every widget that changes a parameter reports it to the host as one automation
gesture. Knobs drag at 0.005 of the range per pixel (Shift: ten times finer)
and reset on double-click; the range slider keeps its handles 1.25x apart.

## Rules the tests enforce

- Text and dim text keep at least 4.5:1 contrast on every surface, and the
  accent and glow keep 3:1 for marks. `TEXT_DIM` is therefore `#8a95a9`;
  HalfSpeedBtw 0.1.0 shipped the darker `#6f7d96`.
- The translucent variants match the design system's `#rrggbbaa` tokens.
- Knob arcs sweep 270° from the bottom left, and `LogScale` round-trips.

The full guide, with usage rules, states and live previews of every control,
is the PluginsBTW UI design system. The reference editor is
[`crates/halfspeedbtw/src/editor.rs`](../halfspeedbtw/src/editor.rs).

## License

GPL-3.0-or-later, like the rest of this repository.
