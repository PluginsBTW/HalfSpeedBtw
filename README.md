# HalfTime

A Linux-native half-speed loop effect (CLAP + VST3), written in Rust on
[nice-plug](https://codeberg.org/RustAudio/nice-plug), the maintained community
successor to nih-plug.

**Status:** the core half-speed effect works and is covered by tests. It hasn't
been listening-tested in a DAW yet.

## How it works

Each loop cycle replays the audio captured since the cycle started, slowed
down by the speed ratio (0.5x, 0.667x or 0.25x). The sample played `t` samples
into a cycle was captured `ratio * t` samples into it, so it is always in the
past. That means **zero added latency**: no lookahead and nothing reported to
the host.

- **Loop length:** 1/16 to 4 bars. While the transport plays with Sync on,
  cycles line up with the host's bar grid and time signature. Seeking or a DAW
  loop wrap-around restarts the cycle at the right offset. When the transport
  is stopped, or Sync is off, loops free-run at the host tempo.
- **Smoothness:** sets the equal-power crossfade at each loop restart, from
  1 ms up to a quarter of the loop.
- **Fade:** sets how the effect fades in and out when switched on or off
  (off = 5 ms, fast = 50 ms, normal = 250 ms, slow = 1 s). While it's off, the
  loop keeps running so it stays in time.
- **Mix:** dry/wet balance, smoothed over 20 ms.
- **Not yet:** reverse playback, multiband processing, and a waveform display.

## Layout

| Path                  | What it is                                                     |
| --------------------- | -------------------------------------------------------------- |
| `crates/halftime-dsp` | Plugin-API-independent DSP engine (no dependencies)            |
| `crates/halftime`     | Plugin shell: parameters, editor, CLAP/VST3 export             |
| `xtask`               | `cargo xtask bundle`, which builds `.clap` / `.vst3` bundles   |

DSP logic belongs in `halftime-dsp`, so it can be tested without a host and kept
independent of the plugin framework.

## Build and test

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets
```

```bash
cargo xtask bundle halftime --release
```

The bundles are written to `target/bundled/`. To install them for your user:

```bash
mkdir -p ~/.clap ~/.vst3 && cp target/bundled/HalfTime.clap ~/.clap/ && cp -r target/bundled/HalfTime.vst3 ~/.vst3/
```

Debug builds enable nice-plug's `assert_process_allocs`, which aborts if
`process()` allocates. The `no_alloc` test checks the engine for heap use
directly.

A rough CPU benchmark (real-time factor and worst block time per block size):

```bash
cargo run --release -p halftime-dsp --example bench
```

## Design decisions

- **Mode:** the first DSP prototype targets beat-synced loops (host tempo,
  1/16 to 4 bars) with smooth crossfades. It does not aim for the lowest
  possible live latency.
- **Real-time rules:** no allocation, locks, logging, or I/O in `process()`.
  Buffers are sized in `initialize`/`activate`.
- **Parameter IDs** (`#[id = "..."]` in `crates/halftime/src/params.rs`) are
  permanent once released. A test fails if they change. The CLAP ID
  `com.romanhofmann.halftime` and the VST3 class ID are permanent too.

## Licensing

HalfTime is licensed under GPL-3.0-or-later (see `LICENSE`).

Every shipped dependency uses a permissive license compatible with GPLv3: MIT,
Apache-2.0, ISC, Zlib, BSD, or Unlicense. This includes nice-plug (ISC) and the
`vst3` bindings (MIT OR Apache-2.0). egui's bundled default fonts are under
OFL-1.1 and the Ubuntu Font Licence. Binary releases must include those
notices.
