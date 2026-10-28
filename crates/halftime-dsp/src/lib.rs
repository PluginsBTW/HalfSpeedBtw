//! Real-time DSP for HalfTime, kept independent of any plugin API.
//!
//! Everything in this crate that is called from the audio thread must be allocation-free and
//! lock-free. Buffers are sized up front through constructors or `resize` methods, which are
//! meant to be called from the plugin's non-real-time `initialize`/`activate` callbacks.

pub mod ring_buffer;

pub use ring_buffer::RingBuffer;
