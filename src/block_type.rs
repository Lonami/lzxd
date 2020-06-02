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
