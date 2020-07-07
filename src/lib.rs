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
mod lzxd;
mod tree;
mod window_size;

pub(crate) use bitstream::Bitstream;
pub(crate) use block::{BlockHead, BlockType};
pub use lzxd::{Lzxd, MAX_CHUNK_SIZE};
pub(crate) use tree::{CanonicalTree, Tree};
pub use window_size::WindowSize;
