use crate::Tree;
use std::convert::TryFrom;

#[derive(Debug, PartialEq, Eq)]
pub enum BlockType {
    Verbatim = 1,
    AlignedOffset = 2,
    Uncompressed = 3,
}

impl TryFrom<u8> for BlockType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            // > Each block of compressed data begins with a 3-bit Block Type field.
            // > Of the eight possible values, only three are valid values for the Block Type
            // > field.
            0b001 => Self::Verbatim,
            0b010 => Self::AlignedOffset,
            0b011 => Self::Uncompressed,
            _ => return Err(()),
        })
    }
}

/// Note that this is not the block header, but the head of the block's body, which includes
/// everything except the tail of the block data (either uncompressed data or token sequence).
#[derive(Debug)]
pub enum BlockHead {
    Verbatim {
        /// Only 24 bits may be used.
        size: u32,
        main_tree: Tree,
        length_tree: Tree,
    },
    AlignedOffset {
        /// Only 24 bits may be used.
        size: u32,
        aligned_offset_tree: Tree,
        main_tree: Tree,
        length_tree: Tree,
    },
    Uncompressed {
        /// Only 24 bits may be used.
        size: u32,
        r: [u32; 3],
    },
}
