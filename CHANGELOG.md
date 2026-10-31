# Changelog

## Unreleased

- **Editor:** the power switch moved from the middle of the display to the
  header's right corner, next to SYNC, so it no longer covers the waveform.
  The version number shows in the bottom-right corner.
- **Performance:** resetting no longer clears the 40-second capture buffer,
  which took up to 1 ms at 192 kHz on the audio thread and could cause a
  dropout; it is now instant. Processing is about 13% faster.
- **Development:** `btw-ui` gains `compact_power_switch` and `version_label`.

## 0.1.1

- **Editor:** captions, unselected labels and the status line use a lighter
  dim text (`#8a95a9`), which keeps 4.5:1 contrast on every surface. The
  layout is unchanged.
- **Development:** the look and the controls moved into a reusable crate,
  `btw-ui`, so other PluginsBTW plugins can share them.

## 0.1.0

The first release of HalfSpeedBtw, a tempo-synced half-speed loop effect for
Linux, as CLAP and VST3.

### Features

- **Slowdown modes:** 1.5x, 2x and 4x slower, with pitch dropping along with
  the speed.
- **Loop lengths:** 1/16, 1/8T, 1/8, 1/4, 1/2, 1, 2, 4 and 8 bars, synced to
  the host's tempo, time signature and bar grid. Loops recover cleanly when
  you seek or the DAW loops back, and free-run while the transport is stopped.
- **Zero added latency:** no lookahead and nothing reported to the host.
- **Power switch** with separate fade in and fade out lengths, from instant
  up to 16 bars.
- **Smooth:** an equal-power crossfade at each loop restart, from 1 ms up to
  a quarter of the loop.
- **Band:** low cut and high cut (12 dB/octave) on the slowed-down signal.
- **Mix:** dry/wet balance.
- **Editor:** dark, blue-accented, with a live waveform of the current loop
  and a playhead.
- **Automation:** all parameters can be automated sample-accurately.
- **Channels:** mono and stereo.

### Download notes

- **Build:** Linux x86-64, linked against glibc 2.31. It runs on Ubuntu
  20.04, Debian 11, Fedora 32 and newer.
- **Editor requirements:** X11 or XWayland, and a Vulkan driver.
- **Tested in:** Bitwig Studio on Arch Linux.
