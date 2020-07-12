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
mod window_size;

pub(crate) use bitstream::Bitstream;
pub(crate) use block::{Block, Decoded, Kind as BlockKind};
use std::fmt;
pub(crate) use tree::{CanonicalTree, Tree};
pub use window_size::WindowSize;

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
    // TODO proper `Window` struct that handles wrap around for us.
    window: Vec<u8>,

    /// Current position into the sliding window.
    pos: usize,

    /// Current decoder state.
    state: DecoderState,

    /// > The three most recent real match offsets are kept in a list.
    r: [u32; 3],

    /// Has the very first chunk been read yet? Unlike the rest, it has additional data.
    first_chunk_read: bool,

    /// This field will update after the first chunk is read, but will remain being `None`
    /// if the E8 Call Translation is not enabled for this stream.
    _e8_translation_size: Option<u32>,

    /// Current block.
    current_block: Block,
}

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

    /// An invalid pretree element was found.
    InvalidPretreeElement(u16),
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
            InvalidPretreeElement(elem) => write!(f, "found invalid pretree element {}", elem),
        }
    }
}

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
            pos: 0,
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
            _e8_translation_size: None,
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
            self._e8_translation_size = if e8_translation {
                let high = bitstream.read_u16_le()? as u32;
                let low = bitstream.read_u16_le()? as u32;
                Some((high << 16) | low)
            } else {
                None
            };

            // We don't support e8 translation yet
            if self._e8_translation_size.is_some() {
                todo!("e8 translation not implemented");
            }
        }

        Ok(())
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

        let start = self.pos;
        while !bitstream.is_empty() {
            if self.current_block.size == 0 {
                self.current_block = Block::read(&mut bitstream, &mut self.state)?;
            }

            let decoded = self
                .current_block
                .decode_element(&mut bitstream, &mut self.r)?;

            let advance = match decoded {
                Decoded::Single(value) => {
                    self.window[self.pos] = value;
                    1
                }
                Decoded::Match { offset, length } => {
                    // TODO this can be improved by avoiding %
                    for i in 0..length {
                        let li = (self.pos + i) % self.window.len();
                        let ri = (self.window.len() + self.pos + i - offset) % self.window.len();
                        self.window[li] = self.window[ri];
                    }
                    length
                }
                Decoded::Read(length) => {
                    bitstream.read_raw(&mut self.window[self.pos..self.pos + length])?;
                    length
                }
            };

            self.pos += advance;
            if let Some(value) = self.current_block.size.checked_sub(advance as u32) {
                self.current_block.size = value;
            } else {
                return Err(DecodeFailed::OverreadBlock);
            }
        }
        let end = self.pos;

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

        self.pos = self.pos % self.window.len();

        // TODO last chunk may misalign this and on the next iteration we wouldn't be able
        // to return a continous slice. if we're called on non-aligned, we could shift things
        // and align it.
        return Ok(&self.window[start..end]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_uncompressed() {
        let data = [
            0x00, 0x30, 0x30, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x61, 0x62, 0x63, 0x00,
        ];

        let mut lzxd = Lzxd::new(WindowSize::KB32); // size does not matter
        let res = lzxd.decompress_next(&data);
        assert_eq!(res.unwrap(), [b'a', b'b', b'c']);
    }
}
