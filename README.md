# HalfTime

A Linux-native half-speed loop effect (CLAP + VST3), written in Rust on
[nice-plug](https://codeberg.org/RustAudio/nice-plug), the maintained community
successor to nih-plug.

**Status:** Phase 1 skeleton. The plugin passes audio through unchanged with
zero added latency, exposes its first parameters, and has a placeholder egui
GUI. The DSP crate has a tested, preallocated ring buffer. No half-speed
processing yet.

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
`process()` allocates.

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
