//! This library implements the LZX compression format as described in
//! [LZX DELTA Compression and Decompression], revision 9.0.
//!
//! Lempel-Ziv Extended (LZX) is an LZ77-based compression engine, as described in [UASDC],
//! that is a universal lossless data compression algorithm. It performs no analysis on the
//! data.
//!
//! Lempel-Ziv Extended Delta (LZXD) is a derivative of the Lempel-Ziv Extended (LZX) format with
//! some modifications to facilitate efficient delta compression.
//!
//! In order to use this module, refer to the main [`Lzxd`] type and its methods.
//!
//! [LZX DELTA Compression and Decompression]: https://docs.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-patch/cc78752a-b4af-4eee-88cb-01f4d8a4c2bf
//! [UASDC]: https://ieeexplore.ieee.org/document/1055714
//! [`Lzxd`]: struct.Lzxd.html
mod bitstream;
mod block;
mod tree;
mod window;

pub(crate) use bitstream::Bitstream;
pub(crate) use block::{Block, Decoded, Kind as BlockKind};
use std::{convert::TryInto, fmt};
pub(crate) use tree::{CanonicalTree, Tree};
use window::Window;
pub use window::WindowSize;

/// A chunk represents exactly 32 KB of uncompressed data until the last chunk in the stream,
/// which can represent less than 32 KB.
pub const MAX_CHUNK_SIZE: usize = 32 * 1024;

/// Decoder state needed for new blocks.
// TODO not sure how much we want to keep in DecoderState and Lzxd respectively
pub(crate) struct DecoderState {
    /// The window size we're working with.
    window_size: WindowSize,

    /// This tree cannot be used directly, it exists only to apply the delta of upcoming trees
    /// to its path lengths.
    main_tree: CanonicalTree,

    /// This tree cannot be used directly, it exists only to apply the delta of upcoming trees
    /// to its path lengths.
    length_tree: CanonicalTree,
}

/// The main interface to perform LZXD decompression.
///
/// This structure stores the required state to process the compressed chunks of data in a
/// sequential order.
///
/// ```no_run
/// # fn get_compressed_chunk() -> Option<Vec<u8>> { unimplemented!() }
/// # fn write_data(a: &[u8]) { unimplemented!() }
/// use ::lzxd::{Lzxd, WindowSize};
///
/// let mut lzxd = Lzxd::new(WindowSize::KB64);
///
/// while let Some(chunk) = get_compressed_chunk() {
///     let decompressed = lzxd.decompress_next(&chunk);
///     write_data(decompressed.unwrap());
/// }
/// ```
pub struct Lzxd {
    /// Sliding window into which data is decompressed.
    window: Window,

    /// Current decoder state.
    state: DecoderState,

    /// > The three most recent real match offsets are kept in a list.
    r: [u32; 3],

    /// The current offset into the decompressed data.
    chunk_offset: usize,

    /// Has the very first chunk been read yet? Unlike the rest, it has additional data.
    first_chunk_read: bool,

    /// This field will update after the first chunk is read, but will remain being `None`
    /// if the E8 Call Translation is not enabled for this stream.
    e8_translation_size: Option<u32>,

    /// Current block.
    current_block: Block,
}

/// The error type used when decompression fails.
#[derive(Debug, PartialEq)]
pub enum DecodeFailed {
    /// The chunk length must be divisible by 2.
    OddLength,

    /// The chunk data caused a read of more items than the current block had in a single step.
    OverreadBlock,

    /// There was not enough data in the chunk to fully decode, and a premature end was found.
    UnexpectedEof,

    /// An invalid block type was found.
    InvalidBlock(u8),

    /// An invalid block size was found.
    InvalidBlockSize(u32),

    /// An invalid pretree element was found.
    InvalidPretreeElement(u16),

    /// Invalid pretree run-length encoding.
    InvalidPretreeRle,

    /// When attempting to construct a decode tree, we encountered an invalid path length tree.
    InvalidPathLengths,

    /// The given window size was too small.
    WindowTooSmall,

    /// Tried to read a chunk longer than [`MAX_CHUNK_SIZE`].
    ///
    /// [`MAX_CHUNK_SIZE`]: constant.MAX_CHUNK_SIZE.html
    ChunkTooLong,
}

impl fmt::Display for DecodeFailed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use DecodeFailed::*;

        match self {
            OddLength => write!(f, "chunk length must be divisible by 2"),
            OverreadBlock => write!(
                f,
                "read more items than available in the block in a single step"
            ),
            UnexpectedEof => write!(f, "reached end of chunk without fully decoding it"),
            InvalidBlock(kind) => write!(f, "block type {} is invalid", kind),
            InvalidBlockSize(size) => write!(f, "block size {} is invalid", size),
            InvalidPretreeElement(elem) => write!(f, "found invalid pretree element {}", elem),
            InvalidPretreeRle => write!(f, "found invalid pretree rle element"),
            InvalidPathLengths => write!(f, "encountered invalid path lengths"),
            WindowTooSmall => write!(f, "decode window was too small"),
            ChunkTooLong => write!(
                f,
                "tried reading a chunk longer than {} bytes",
                MAX_CHUNK_SIZE
            ),
        }
    }
}

impl std::error::Error for DecodeFailed {}

impl Lzxd {
    /// Creates a new instance of the LZXD decoder state. The [`WindowSize`] must be obtained
    /// from elsewhere (e.g. it may be predetermined to a certain value), and if it's wrong,
    /// the decompressed values won't be those expected.
    ///
    /// [`WindowSize`]: enum.WindowSize.html
    pub fn new(window_size: WindowSize) -> Self {
        // > The main tree comprises 256 elements that correspond to all possible 8-bit
        // > characters, plus 8 * NUM_POSITION_SLOTS elements that correspond to matches.
        let main_tree = CanonicalTree::new(256 + 8 * window_size.position_slots());

        // > The length tree comprises 249 elements.
        let length_tree = CanonicalTree::new(249);

        Self {
            window: window_size.create_buffer(),
            // > Because trees are output several times during compression of large amounts of
            // > data (multiple blocks), LZXD optimizes compression by encoding only the delta
            // > path lengths lengths between the current and previous trees.
            //
            // Because it uses deltas, we need to store the previous value across blocks.
            state: DecoderState {
                window_size,
                main_tree,
                length_tree,
            },
            // > The initial state of R0, R1, R2 is (1, 1, 1).
            r: [1, 1, 1],
            first_chunk_read: false,
            chunk_offset: 0,
            e8_translation_size: None,
            // Start with some dummy value.
            current_block: Block {
                size: 0,
                kind: BlockKind::Uncompressed { r: [1, 1, 1] },
            },
        }
    }

    /// Try reading the header for the first chunk.
    fn try_read_first_chunk(&mut self, bitstream: &mut Bitstream) -> Result<(), DecodeFailed> {
        // > The first bit in the first chunk in the LZXD bitstream (following the 2-byte,
        // > chunk-size prefix described in section 2.2.1) indicates the presence or absence of
        // > two 16-bit fields immediately following the single bit. If the bit is set, E8
        // > translation is enabled.
        if !self.first_chunk_read {
            self.first_chunk_read = true;

            let e8_translation = bitstream.read_bit()? != 0;
            self.e8_translation_size = if e8_translation {
                let high = bitstream.read_u16_le()? as u32;
                let low = bitstream.read_u16_le()? as u32;
                Some((high << 16) | low)
            } else {
                None
            };
        }

        Ok(())
    }

    /// Attempts to perform post-decompression E8 fixups on an output data buffer.
    fn postprocess<'a>(
        e8_translation_size: Option<u32>,
        chunk_offset: usize,
        idata: &'a mut [u8],
    ) -> Result<&'a [u8], DecodeFailed> {
        let translation_size = match e8_translation_size {
            // E8 fixups are disabled after 1GB of input data,
            // or if the chunk size is too small.
            Some(size) if chunk_offset < 0x4000_0000 && idata.len() > 10 => size,
            _ => return Ok(idata),
        };

        let mut processed = 0usize;

        // Find the next E8 match, or finish once there are no more E8 matches.
        while let Some(pos) = idata[processed..]
            .iter()
            .position(|&e| e == 0xE8)
            .map(|pos| processed + pos)
        {
            // N.B: E8 fixups are only performed for up to 10 bytes before the end of a chunk.
            if idata.len() - pos < 10 {
                break;
            }

            // This is the current file output pointer.
            let current_pointer = chunk_offset + pos;

            // Match. Fix up the following bytes.
            // N.B: 4-byte slice conversion to an array will not fail.
            let abs_val = u32::from_le_bytes(idata[pos + 1..pos + 5].try_into().unwrap());

            if (abs_val as usize) < current_pointer && (abs_val as u32) < translation_size {
                let rel_val = if (abs_val as i32).is_positive() {
                    abs_val.wrapping_sub(current_pointer as u32) as i32
                } else {
                    abs_val.wrapping_add(translation_size) as i32
                };

                idata[pos + 1..pos + 5].copy_from_slice(&rel_val.to_le_bytes());
            }

            processed = pos + 5;
        }

        Ok(idata)
    }

    /// Decompresses the next compressed `chunk` from the LZXD data stream.
    pub fn decompress_next(&mut self, chunk: &[u8]) -> Result<&[u8], DecodeFailed> {
        // > A chunk represents exactly 32 KB of uncompressed data until the last chunk in the
        // > stream, which can represent less than 32 KB.
        //
        // > The LZXD engine encodes a compressed, chunk-size prefix field preceding each
        // > compressed chunk in the compressed byte stream. The compressed, chunk-size prefix
        // > field is a byte aligned, little-endian, 16-bit field.
        //
        // However, this doesn't seem to be part of LZXD itself? At least when testing with
        // `.xnb` files, every chunk comes with a compressed chunk size unless it has the flag
        // set to 0xff where it also includes the uncompressed chunk size.
        //
        // TODO maybe the docs could clarify whether this length is compressed or not
        if chunk.len() % 2 != 0 {
            return Err(DecodeFailed::OddLength);
        }

        let mut bitstream = Bitstream::new(chunk);

        self.try_read_first_chunk(&mut bitstream)?;

        let mut decoded_len = 0;
        while !bitstream.is_empty() {
            if self.current_block.size == 0 {
                self.current_block = Block::read(&mut bitstream, &mut self.state)?;
                assert!(self.current_block.size != 0);
            }

            let decoded = self
                .current_block
                .decode_element(&mut bitstream, &mut self.r)?;

            let advance = match decoded {
                Decoded::Single(value) => {
                    self.window.push(value);
                    1
                }
                Decoded::Match { offset, length } => {
                    self.window.copy_from_self(offset, length);
                    length
                }
                Decoded::Read(length) => {
                    // Will re-align if needed, just as decompressed reads mandate.
                    self.window.copy_from_bitstream(&mut bitstream, length)?;
                    length
                }
            };

            assert!(advance != 0);
            decoded_len += advance;
            if let Some(value) = self.current_block.size.checked_sub(advance as u32) {
                self.current_block.size = value;
            } else {
                return Err(DecodeFailed::OverreadBlock);
            }
        }

        // > To ensure that an exact number of input bytes represent an exact number of
        // > output bytes for each chunk, after each 32 KB of uncompressed data is
        // > represented in the output compressed bitstream, the output bitstream is padded
        // > with up to 15 bits of zeros to realign the bitstream on a 16-bit boundary
        // > (even byte boundary) for the next 32 KB of data. This results in a compressed
        // > chunk of a byte-aligned size. The compressed chunk could be smaller than 32 KB
        // > or larger than 32 KB if the data is incompressible when the chunk is not the
        // > last one.
        //
        // That's the input chunk parsed which aligned to a byte-boundary already. There is
        // no need to align the bitstream because on the next call it will be aligned.

        // TODO last chunk may misalign this and on the next iteration we wouldn't be able
        // to return a continous slice. if we're called on non-aligned, we could shift things
        // and align it.

        // FIXME: Why is the last block's size observed to be one?
        if self.current_block.size > 1 {
            // Align the window up to 32KB.
            // See https://github.com/Lonami/lzxd/issues/7 for details.
            if let Some(len) = 0x8000usize.checked_sub(decoded_len) {
                self.window.zero_extend(len);
                decoded_len += len;
            }
        }

        // Finally, postprocess the output buffer (if necessary).
        let res = Self::postprocess(
            self.e8_translation_size,
            self.chunk_offset,
            self.window.past_view(decoded_len)?,
        );

        self.chunk_offset += decoded_len;
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_uncompressed() {
        let data = [
            0x00, 0x30, 0x30, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, b'a', b'b', b'c', 0x00,
        ];

        let mut lzxd = Lzxd::new(WindowSize::KB32); // size does not matter
        let res = lzxd.decompress_next(&data);
        assert_eq!(res.unwrap(), [b'a', b'b', b'c']);
    }

    #[test]
    fn check_e8() {
        let data = [
            0x5B, 0x80, 0x80, 0x8D, 0x00, 0x30, 0x80, 0x0A, 0x18, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x54, 0x68, 0x69, 0x73, 0x20, 0x66, 0x69, 0x6C,
            0x65, 0x20, 0x68, 0x61, 0x73, 0x20, 0x61, 0x6E, 0x20, 0x45, 0x38, 0x20, 0x62, 0x79,
            0x74, 0x65, 0x20, 0x74, 0x6F, 0x20, 0x74, 0x65, 0x73, 0x74, 0x20, 0x45, 0x38, 0x20,
            0x74, 0x72, 0x61, 0x6E, 0x73, 0x6C, 0x61, 0x74, 0x69, 0x6F, 0x6E, 0x2C, 0x20, 0x58,
            0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64,
            0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64,
            0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64,
            0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64,
            0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64,
            0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64,
            0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64, 0xE8, 0x7B,
            0x00, 0x00, 0x00, 0xE8, 0x7B, 0x00, 0x00, 0x00, 0x64, 0x64, 0x64, 0x64, 0x64, 0x64,
            0x64, 0x64, 0x64, 0x64, 0x64, 0x64,
        ];

        let mut lzxd = Lzxd::new(WindowSize::KB32);
        let res = lzxd.decompress_next(&data);
        assert_eq!(
            res.unwrap(),
            b"This file has an E8 byte to test E8 translation, Xdddddddddddddddd\
              dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd\
              dddddddddddddd\xE8\xE9\xFF\xFF\xFF\xE8\xE4\xFF\xFF\xFFdddddddddddd"
        );
    }
}
