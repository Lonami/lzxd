//! This library implements the LZX compression format as described in
//! [LZX DELTA Compression and Decompression], revision 9.0.
//!
//! Lempel-Ziv Extended (LZX) is an LZ77-based compression engine, as described in [UASDC],
//! that is a universal lossless data compression algorithm. It performs no analysis on the
//! data.
//!
//! Lempel-Ziv Extended Delta (LZXD) is a derivative of the Lempel-Ziv Extended (LZX) format with
//! some modifications to facilitate efficient delta compression. Delta compression is a technique
//! in which one set of data can be compressed within the context of a reference set of data that
//! is supplied both to the compressor and decompressor. Delta compression is commonly used to
//! encode updates to similar existing data sets so that the size of compressed data can be
//! significantly reduced relative to ordinary non-delta compression techniques. Expanding a
//! delta-compressed set of data requires that the exact same reference data be provided during
//! decompression.
//!
//! [LZX DELTA Compression and Decompression]: https://docs.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-patch/cc78752a-b4af-4eee-88cb-01f4d8a4c2bf
//! [UASDC]: https://ieeexplore.ieee.org/document/1055714
mod bitstream;
mod block;
mod lzxd;
mod tree;
mod window_size;

pub(crate) use bitstream::Bitstream;
pub(crate) use block::{BlockHead, BlockType};
pub use lzxd::Lzxd;
pub(crate) use tree::Tree;
pub use window_size::WindowSize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run() {
        const DATA: &[u8] = include_bytes!("../a.lzxd");

        let mut lzxd = Lzxd::new(WindowSize::KB64, DATA);
        while let Some(block) = lzxd.next_block() {
            dbg!(block);
        }
    }
}
