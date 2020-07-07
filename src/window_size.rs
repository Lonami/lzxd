/// The window size is not stored in the compressed data stream and must be known before
/// decoding begins.
///
/// The window size should be the smallest power of two between 2^17 and 2^25 that is greater
/// than or equal to the sum of the size of the reference data rounded up to a multiple of
/// 32_768 and the size of the subject data. However, some implementations also seem to support
/// a window size of less than 2^17, and this one is no exception.
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

    pub(crate) fn create_buffer(&self) -> Vec<u8> {
        vec![0; self.value()]
    }
}
