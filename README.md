# HalfSpeedBtw

![HalfSpeedBtw editor](docs/screenshot.png)

A Linux-native slowdown loop effect (CLAP + VST3), written in Rust on
[nice-plug](https://codeberg.org/RustAudio/nice-plug), the maintained community
successor to nih-plug.

**Status:** the core half-speed effect works and is covered by tests. It hasn't
been listening-tested in a DAW yet.

## How it works

Each loop cycle replays the audio captured since the cycle started, slowed
down by the mode (1.5x, 2x or 4x slower). The sample played `t` samples into a
cycle was captured `t / slowdown` samples into it, so it is always in the
past. That means **zero added latency**: no lookahead and nothing reported to
the host.

- **Loop:** 1/16, 1/8T, 1/8, 1/4, 1/2, 1, 2, 4 or 8 bars. While the transport
  plays with Sync on, cycles line up with the host's bar grid and time
  signature. Seeking or a DAW loop wrap-around restarts the cycle at the right
  offset. When the transport is stopped, or Sync is off, loops free-run at the
  host tempo.
- **Power switch** (center of the display): switches the effect on and off.
  While it's off, the loop keeps running so it stays in time.
- **Fade In / Fade Out:** how long the effect takes to fade in or out when
  switched, from instant up to 16 bars.
- **Smooth:** sets the equal-power crossfade at each loop restart, from 1 ms
  up to a quarter of the loop.
- **Band:** a low cut and a high cut (12 dB/octave state-variable filters) on
  the slowed-down signal. At their end stops the filters are fully bypassed.
- **Mix:** dry/wet balance, smoothed over 20 ms.
- **Display:** the slowed-down signal's level across the current cycle, with
  a playhead.
- **Not yet:** reverse playback and per-band processing.

## Controls

Drag knobs up or down; hold Shift for fine control. Double-click a knob or a
band handle to reset it.

## Layout

| Path                  | What it is                                                     |
| --------------------- | -------------------------------------------------------------- |
| `crates/halfspeed-dsp` | Plugin-API-independent DSP engine (no dependencies)            |
| `crates/halfspeedbtw` | Plugin shell: parameters, editor, CLAP/VST3 export             |
| `xtask`               | `cargo xtask bundle`, which builds `.clap` / `.vst3` bundles   |

DSP logic belongs in `halfspeed-dsp`, so it can be tested without a host and kept
independent of the plugin framework.

## Build and test

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets
```

```bash
cargo xtask bundle halfspeedbtw --release
```

The bundles are written to `target/bundled/`. To install them for your user:

```bash
mkdir -p ~/.clap ~/.vst3 && cp target/bundled/HalfSpeedBtw.clap ~/.clap/ && cp -r target/bundled/HalfSpeedBtw.vst3 ~/.vst3/
```

Debug builds enable nice-plug's `assert_process_allocs`, which aborts if
`process()` allocates. The `no_alloc` test checks the engine for heap use
directly.

To work on the GUI without a DAW, run it as a standalone app:

```bash
cargo run -p halfspeedbtw --features standalone -- --backend dummy
```

A rough CPU benchmark (real-time factor and worst block time per block size):

```bash
cargo run --release -p halfspeed-dsp --example bench
```

## Design decisions

- **Mode:** the first DSP prototype targets beat-synced loops (host tempo,
  1/16 to 8 bars) with smooth crossfades. It does not aim for the lowest
  possible live latency.
- **Real-time rules:** no allocation, locks, logging, or I/O in `process()`.
  Buffers are sized in `initialize`/`activate`.
- **Parameter IDs** (`#[id = "..."]` in `crates/halfspeedbtw/src/params.rs`) are
  permanent once released. A test fails if they change. The CLAP ID
  `com.romanhofmann.halfspeedbtw` and the VST3 class ID are permanent too.

## Rendering

The editor renders through wgpu (Vulkan on Linux), not nice-plug-egui's
default OpenGL renderer. On NVIDIA's proprietary driver (seen with 615.78),
the OpenGL renderer segfaulted when the window opened. baseview calls
`eglGetDisplay` on an X11 connection without naming the platform, and NVIDIA's
EGL takes it for a Wayland display.

## Licensing

HalfSpeedBtw is licensed under GPL-3.0-or-later (see `LICENSE`).

Every shipped dependency uses a permissive license compatible with GPLv3: MIT,
Apache-2.0, ISC, Zlib, BSD, or Unlicense. This includes nice-plug (ISC) and the
`vst3` bindings (MIT OR Apache-2.0). egui's bundled default fonts are under
OFL-1.1 and the Ubuntu Font Licence. Binary releases must include those
notices.
