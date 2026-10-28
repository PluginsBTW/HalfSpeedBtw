//! Fixed-capacity, multichannel circular buffer for captured input audio.

/// A preallocated circular buffer holding the most recent `capacity` frames of audio for each
/// channel.
///
/// Samples are addressed by *age*: age `0` is the most recently written frame, age `1` the one
/// before it, and so on up to `capacity - 1`. Reads past what has actually been written return
/// silence, so a freshly cleared buffer behaves like one filled with zeros.
///
/// Only [`RingBuffer::new`] and [`RingBuffer::resize`] allocate. Every other method is
/// real-time safe.
#[derive(Debug, Clone)]
pub struct RingBuffer {
    /// Channel-major storage: channel `c` occupies `data[c * capacity..(c + 1) * capacity]`.
    data: Vec<f32>,
    num_channels: usize,
    capacity: usize,
    /// Index of the slot the next frame will be written to.
    write_pos: usize,
}

impl RingBuffer {
    /// Allocates a zeroed buffer. `capacity` is clamped to at least one frame.
    pub fn new(num_channels: usize, capacity: usize) -> Self {
        let mut buffer = Self {
            data: Vec::new(),
            num_channels: 0,
            capacity: 0,
            write_pos: 0,
        };
        buffer.resize(num_channels, capacity);
        buffer
    }

    /// Reallocates the buffer for a new layout and clears it. Not real-time safe.
    pub fn resize(&mut self, num_channels: usize, capacity: usize) {
        let capacity = capacity.max(1);
        self.data.clear();
        self.data.resize(num_channels * capacity, 0.0);
        self.num_channels = num_channels;
        self.capacity = capacity;
        self.write_pos = 0;
    }

    /// Zeroes all stored audio without reallocating.
    pub fn clear(&mut self) {
        self.data.fill(0.0);
        self.write_pos = 0;
    }

    pub fn num_channels(&self) -> usize {
        self.num_channels
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Writes one frame, one sample per channel. Extra samples are ignored and missing channels
    /// are written as silence, so a layout mismatch can never index out of bounds.
    pub fn push_frame(&mut self, frame: &[f32]) {
        for channel in 0..self.num_channels {
            let sample = frame.get(channel).copied().unwrap_or(0.0);
            self.data[channel * self.capacity + self.write_pos] = sample;
        }
        self.write_pos = (self.write_pos + 1) % self.capacity;
    }

    /// Writes the same number of frames from per-channel slices, as handed out by a host block.
    ///
    /// The block length is taken from the first channel; shorter channels are padded with
    /// silence. If the block is longer than the capacity, only its last `capacity` frames are
    /// retained, exactly as if the frames had been pushed one by one.
    pub fn push_block(&mut self, channels: &[&[f32]]) {
        let Some(block_len) = channels.first().map(|c| c.len()) else {
            return;
        };

        let skip = block_len.saturating_sub(self.capacity);
        let len = block_len - skip;
        let start = (self.write_pos + skip) % self.capacity;
        // The copy splits into at most two contiguous runs around the wrap point.
        let first_run = len.min(self.capacity - start);

        for channel in 0..self.num_channels {
            let storage = &mut self.data[channel * self.capacity..(channel + 1) * self.capacity];
            let input = channels.get(channel).copied().unwrap_or(&[]);
            let sample_at = |i: usize| input.get(skip + i).copied().unwrap_or(0.0);

            for (i, slot) in storage[start..start + first_run].iter_mut().enumerate() {
                *slot = sample_at(i);
            }
            for (i, slot) in storage[..len - first_run].iter_mut().enumerate() {
                *slot = sample_at(first_run + i);
            }
        }

        self.write_pos = (self.write_pos + block_len) % self.capacity;
    }

    /// Returns the sample written `age` frames ago on `channel`, or silence if either is out of
    /// range.
    pub fn get(&self, channel: usize, age: usize) -> f32 {
        if channel >= self.num_channels || age >= self.capacity {
            return 0.0;
        }

        // `write_pos` points one past the newest frame, so age 0 sits at `write_pos - 1`.
        let index = (self.write_pos + self.capacity - 1 - age) % self.capacity;
        self.data[channel * self.capacity + index]
    }

    /// Reads at a fractional age using linear interpolation between neighbouring frames. Used
    /// by playback-rate engines that step through the buffer at non-integer speeds. The age is
    /// `f64` because multi-second loops need sub-sample precision beyond what `f32` offers.
    pub fn get_interpolated(&self, channel: usize, age: f64) -> f32 {
        if age.is_nan() || age < 0.0 {
            return 0.0;
        }

        let whole = age.floor();
        let frac = (age - whole) as f32;
        let whole = whole as usize;
        let newer = self.get(channel, whole);
        let older = self.get(channel, whole + 1);
        newer + (older - newer) * frac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(len: usize, offset: f32) -> Vec<f32> {
        (0..len).map(|i| offset + i as f32).collect()
    }

    #[test]
    fn new_buffer_reads_silence() {
        let buffer = RingBuffer::new(2, 8);
        for age in 0..8 {
            assert_eq!(buffer.get(0, age), 0.0);
            assert_eq!(buffer.get(1, age), 0.0);
        }
    }

    #[test]
    fn zero_capacity_is_clamped() {
        let mut buffer = RingBuffer::new(1, 0);
        assert_eq!(buffer.capacity(), 1);
        buffer.push_frame(&[3.0]);
        assert_eq!(buffer.get(0, 0), 3.0);
    }

    #[test]
    fn ages_count_back_from_newest_frame() {
        let mut buffer = RingBuffer::new(1, 4);
        for sample in [1.0, 2.0, 3.0] {
            buffer.push_frame(&[sample]);
        }
        assert_eq!(buffer.get(0, 0), 3.0);
        assert_eq!(buffer.get(0, 1), 2.0);
        assert_eq!(buffer.get(0, 2), 1.0);
        assert_eq!(buffer.get(0, 3), 0.0);
    }

    #[test]
    fn wraparound_keeps_latest_capacity_frames() {
        let mut buffer = RingBuffer::new(1, 4);
        for sample in ramp(11, 1.0) {
            buffer.push_frame(&[sample]);
        }
        // Frames 1..=11 were written; only 8..=11 fit.
        let expected = [11.0, 10.0, 9.0, 8.0];
        for (age, value) in expected.into_iter().enumerate() {
            assert_eq!(buffer.get(0, age), value);
        }
    }

    #[test]
    fn out_of_range_reads_are_silent() {
        let mut buffer = RingBuffer::new(1, 4);
        buffer.push_frame(&[1.0]);
        assert_eq!(buffer.get(0, 4), 0.0);
        assert_eq!(buffer.get(0, usize::MAX), 0.0);
        assert_eq!(buffer.get(1, 0), 0.0);
    }

    #[test]
    fn channels_are_independent() {
        let mut buffer = RingBuffer::new(2, 4);
        buffer.push_frame(&[1.0, -1.0]);
        buffer.push_frame(&[2.0, -2.0]);
        assert_eq!(buffer.get(0, 0), 2.0);
        assert_eq!(buffer.get(1, 0), -2.0);
        assert_eq!(buffer.get(0, 1), 1.0);
        assert_eq!(buffer.get(1, 1), -1.0);
    }

    #[test]
    fn short_frames_are_padded_with_silence() {
        let mut buffer = RingBuffer::new(2, 4);
        buffer.push_frame(&[5.0, 6.0]);
        buffer.push_frame(&[7.0]);
        assert_eq!(buffer.get(0, 0), 7.0);
        assert_eq!(buffer.get(1, 0), 0.0);
        assert_eq!(buffer.get(1, 1), 6.0);
    }

    #[test]
    fn push_block_matches_push_frame_for_any_block_size() {
        let capacity = 7;
        let input_l = ramp(50, 0.0);
        let input_r = ramp(50, 1000.0);

        let mut reference = RingBuffer::new(2, capacity);
        for (l, r) in input_l.iter().zip(&input_r) {
            reference.push_frame(&[*l, *r]);
        }

        // Includes block sizes that are smaller than, equal to and larger than the capacity.
        for block_size in [1, 3, 6, 7, 8, 13, 50] {
            let mut buffer = RingBuffer::new(2, capacity);
            for (l, r) in input_l.chunks(block_size).zip(input_r.chunks(block_size)) {
                buffer.push_block(&[l, r]);
            }
            for channel in 0..2 {
                for age in 0..capacity {
                    assert_eq!(
                        buffer.get(channel, age),
                        reference.get(channel, age),
                        "block size {block_size}, channel {channel}, age {age}"
                    );
                }
            }
        }
    }

    #[test]
    fn push_block_handles_missing_channels_and_empty_input() {
        let mut buffer = RingBuffer::new(2, 4);
        buffer.push_block(&[]);
        buffer.push_block(&[&[1.0, 2.0]]);
        assert_eq!(buffer.get(0, 0), 2.0);
        assert_eq!(buffer.get(1, 0), 0.0);
        assert_eq!(buffer.get(0, 2), 0.0);
    }

    #[test]
    fn clear_zeroes_without_changing_layout() {
        let mut buffer = RingBuffer::new(2, 4);
        buffer.push_frame(&[1.0, 1.0]);
        buffer.clear();
        assert_eq!((buffer.num_channels(), buffer.capacity()), (2, 4));
        assert_eq!(buffer.get(0, 0), 0.0);
        assert_eq!(buffer.get(1, 3), 0.0);
    }

    #[test]
    fn resize_changes_layout_and_clears() {
        let mut buffer = RingBuffer::new(1, 4);
        buffer.push_frame(&[1.0]);
        buffer.resize(2, 16);
        assert_eq!((buffer.num_channels(), buffer.capacity()), (2, 16));
        assert_eq!(buffer.get(0, 0), 0.0);
    }

    #[test]
    fn interpolated_reads_blend_neighbouring_frames() {
        let mut buffer = RingBuffer::new(1, 8);
        for sample in [0.0, 10.0, 20.0] {
            buffer.push_frame(&[sample]);
        }
        assert_eq!(buffer.get_interpolated(0, 0.0), 20.0);
        assert_eq!(buffer.get_interpolated(0, 0.5), 15.0);
        assert_eq!(buffer.get_interpolated(0, 1.25), 7.5);
        assert_eq!(buffer.get_interpolated(0, -1.0), 0.0);
        assert_eq!(buffer.get_interpolated(0, f64::NAN), 0.0);
    }
}
