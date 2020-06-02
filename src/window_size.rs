/// The window size is not stored in the compressed data stream and MUST be  specified to the
/// decoder before decoding begins.
///
/// The window size SHOULD be the smallest power of two between 2^17 and 2^25 that is greate
/// than or equal to the sum of the size of the reference data rounded up to a multiple of
/// 32_768 and the size of the subject data.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum WindowSize {
    KB32 = 0x0000_8000,
    KB64 = 0x0001_0000,
    KB128 = 0x0002_0000,
    KB256 = 0x0004_0000,
    KB512 = 0x0008_0000,
    MB1 = 0x0010_0000,
    MB2 = 0x0020_0000,
    MB4 = 0x0040_0000,
    MB8 = 0x0080_0000,
    MB16 = 0x0100_0000,
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
