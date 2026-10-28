//! Verifies that the real-time path never touches the heap. Lives in its own test binary
//! because it installs a global allocator.

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use halftime_dsp::{HalfSpeedEngine, Settings, Timing};

struct CountingAllocator;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::SeqCst) {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNTING.load(Ordering::SeqCst) {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::SeqCst) {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn process_and_reset_do_not_allocate() {
    let sample_rate = 48_000.0;
    let mut engine = HalfSpeedEngine::new(sample_rate, 2, 20.0);
    let mut left = vec![0.25; 256];
    let mut right = vec![-0.25; 256];
    let variations = [
        Settings::default(),
        Settings {
            ratio: 0.25,
            loop_beats: 0.25,
            smoothness: 1.0,
            ..Settings::default()
        },
        Settings {
            enabled: false,
            fade_seconds: 1.0,
            mix: 0.5,
            ..Settings::default()
        },
    ];

    COUNTING.store(true, Ordering::SeqCst);
    let mut beat = 0.0;
    for settings in &variations {
        for block_size in [32, 64, 128, 256] {
            for _ in 0..200 {
                let timing = Timing {
                    tempo_bpm: 140.0,
                    beat: Some(beat),
                };
                engine.process(
                    &mut [&mut left[..block_size], &mut right[..block_size]],
                    settings,
                    &timing,
                );
                beat += block_size as f64 / sample_rate as f64 * 140.0 / 60.0;
            }
        }
        engine.process(&mut [&mut left, &mut right], settings, &Timing::default());
    }
    engine.reset();
    // Same layout again, as hosts do when restoring state: must not reallocate either.
    engine.prepare(sample_rate, 2, 20.0);
    COUNTING.store(false, Ordering::SeqCst);

    assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 0);
}
