/// The window size is not stored in the compressed data stream and must be known before
/// decoding begins.
///
/// The window size should be the smallest power of two between 2^17 and 2^25 that is greater
/// than or equal to the sum of the size of the reference data rounded up to a multiple of
/// 32_768 and the size of the subject data. However, some implementations also seem to support
/// a window size of less than 2^17, and this one is no exception.
use crate::{Bitstream, DecodeFailed, MAX_CHUNK_SIZE};

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum WindowSize {
    /// Window size of 32 KB (2^15 bytes).
    KB32 = 0x0000_8000,
    /// Window size of 64 KB (2^16 bytes).
    KB64 = 0x0001_0000,
    /// Window size of 128 KB (2^17 bytes).
    KB128 = 0x0002_0000,
    /// Window size of 256 KB (2^18 bytes).
    KB256 = 0x0004_0000,
    /// Window size of 512 KB (2^19 bytes).
    KB512 = 0x0008_0000,
    /// Window size of 1 MB (2^20 bytes).
    MB1 = 0x0010_0000,
    /// Window size of 2 MB (2^21 bytes).
    MB2 = 0x0020_0000,
    /// Window size of 4 MB (2^22 bytes).
    MB4 = 0x0040_0000,
    /// Window size of 8 MB (2^23 bytes).
    MB8 = 0x0080_0000,
    /// Window size of 16 MB (2^24 bytes).
    MB16 = 0x0100_0000,
    /// Window size of 32 MB (2^25 bytes).
    MB32 = 0x0200_0000,
}

/// A sliding window of a certain size.
///
/// A `std::collections::VecDeque` is not used because the `deque_make_contiguous` feature
/// is [nightly-only experimental](https://github.com/rust-lang/rust/issues/70929).
pub struct Window {
    pos: usize,
    buffer: Box<[u8]>,
}

impl WindowSize {
    /// The window size determines the number of window subdivisions, or position slots.
    pub(crate) fn position_slots(&self) -> usize {
        use WindowSize::*;

        match self {
            KB32 => 30,
            KB64 => 32,
            KB128 => 34,
            KB256 => 36,
            KB512 => 38,
            MB1 => 42,
            MB2 => 50,
            MB4 => 66,
            MB8 => 98,
            MB16 => 162,
            MB32 => 290,
        }
    }

    fn value(&self) -> usize {
        *self as usize
    }

    pub(crate) fn create_buffer(&self) -> Window {
        // The window must be at least as big as the smallest chunk, or else we can't possibly
        // contain an entire chunk inside of the sliding window.
        assert!(self.value() >= MAX_CHUNK_SIZE);

        Window {
            pos: 0,
            buffer: vec![0; self.value()].into_boxed_slice(),
        }
    }
}

impl Window {
    fn advance(&mut self, delta: usize) {
        self.pos += delta;
        if self.pos >= self.buffer.len() {
            self.pos -= self.buffer.len();
        }
    }

    pub fn push(&mut self, value: u8) {
        self.buffer[self.pos] = value;
        self.advance(1);
    }

    pub fn copy_from_self(&mut self, offset: usize, length: usize) {
        // TODO this can be improved by avoiding %
        for i in 0..length {
            let li = (self.pos + i) % self.buffer.len();
            let ri = (self.buffer.len() + self.pos + i - offset) % self.buffer.len();
            self.buffer[li] = self.buffer[ri];
        }
        self.advance(length);
    }

    pub fn copy_from_bitstream(
        &mut self,
        bitstream: &mut Bitstream,
        length: usize,
    ) -> Result<(), DecodeFailed> {
        // TODO test reading at boundary
        bitstream.read_raw(&mut self.buffer[self.pos..self.pos + length])?;
        self.advance(length);
        Ok(())
    }

    pub fn past_view(&self, len: usize) -> Result<&[u8], DecodeFailed> {
        if len < MAX_CHUNK_SIZE {
            Ok(&self.buffer[self.pos - len..self.pos])
        } else {
            Err(DecodeFailed::ChunkTooLong)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_push() {
        let mut window = WindowSize::KB32.create_buffer();
        window.push(1);
        window.push(2);
        window.push(3);
        assert_eq!(window.past_view(3).unwrap(), &[1, 2, 3]);
        // TODO test at end of window
    }

    #[test]
    fn check_copy_from_self() {
        let mut window = WindowSize::KB32.create_buffer();
        window.push(1);
        window.push(2);
        window.push(3);
        window.copy_from_self(3, 2);
        assert_eq!(window.past_view(5).unwrap(), &[1, 2, 3, 1, 2]);
        // TODO test at end of window
    }

    #[test]
    fn check_past_view() {
        let mut window = WindowSize::KB32.create_buffer();
        window.push(1);
        window.push(2);
        window.push(3);
        assert_eq!(window.past_view(2).unwrap(), &[2, 3]);
        assert_eq!(window.past_view(3).unwrap(), &[1, 2, 3]);
        // TODO test at end of window
    }

    #[test]
    fn check_past_view_too_long() {
        let window = WindowSize::KB32.create_buffer();
        assert_eq!(
            window.past_view(1 << 15 + 1),
            Err(DecodeFailed::ChunkTooLong)
        );
    }
}
