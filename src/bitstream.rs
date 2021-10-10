/// > An LZXD bitstream is encoded as a sequence of aligned 16-bit integers stored in the
/// > least-significant- byte to most-significant-byte order, also known as byte-swapped,
/// > or little-endian, words. Given an input stream of bits named a, b, c,..., x, y, z,
/// > A, B, C, D, E, F, the output byte stream MUST be as [ 0| 1| 2| 3|...|30|31].
///
/// It is worth mentioning that older revisions of the document explain this better:
///
/// > Given an input stream of bits named a, b, c, ..., x, y, z, A, B, C, D, E, F, the output
/// > byte stream (with byte boundaries highlighted) would be as follows:
/// > [i|j|k|l|m|n|o#p|a|b|c|d|e|f|g|h#y|z|A|B|C|D|E|F#q|r|s|t|u|v|w|x]
use crate::DecodeFailed;

pub struct Bitstream<'a> {
    buffer: &'a [u8],
    // Next number in the bitstream.
    n: u16,
    // How many bits left in the current `n`.
    remaining: u8,
}

impl<'a> Bitstream<'a> {
    /// # Panics
    ///
    /// Panics if `buffer` is not evenly divisible.
    pub fn new(buffer: &'a [u8]) -> Self {
        if buffer.len() % 2 != 0 {
            panic!("bitstream buffer must be evenly divisible");
        }

        Self {
            buffer,
            n: 0,
            remaining: 0,
        }
    }

    // Advance the buffer to the next 16-bit integer.
    fn advance_buffer(&mut self) -> Result<(), DecodeFailed> {
        if self.buffer.is_empty() {
            return Err(DecodeFailed::UnexpectedEof);
        }

        self.remaining = 16;
        self.n = u16::from_le_bytes([self.buffer[0], self.buffer[1]]);
        self.buffer = &self.buffer[2..];
        Ok(())
    }

    pub fn read_bit(&mut self) -> Result<u16, DecodeFailed> {
        if self.remaining == 0 {
            self.advance_buffer()?;
        }

        self.remaining -= 1;
        self.n = self.n.rotate_left(1);
        Ok(self.n & 1)
    }

    /// Read from the bistream, no more than 16 bits (one word).
    fn read_bits_oneword(&mut self, bits: u8) -> Result<u16, DecodeFailed> {
        assert!(bits <= 16);
        debug_assert!(self.remaining <= 16);

        Ok(if bits <= self.remaining {
            self.remaining -= bits;
            self.n = self.n.rotate_left(bits as u32);
            self.n & ((1 << bits) - 1)
        } else {
            // No need to store `rol` result in `n` as we're about to overwrite it.
            let hi = self.n.rotate_left(self.remaining as u32) & ((1 << self.remaining) - 1);
            let bits = bits - self.remaining;
            self.advance_buffer()?;

            self.remaining -= bits;
            self.n = self.n.rotate_left(bits as u32);
            // `bits` may be 16 which would overflow the left shift, operate on `u32` and trunc.
            let lo = self.n & ((1u32 << bits) as u16).wrapping_sub(1);

            ((hi as u32) << bits) as u16 | lo
        })
    }

    pub fn read_bits(&mut self, bits: u8) -> Result<u32, DecodeFailed> {
        if bits <= 16 {
            self.read_bits_oneword(bits).map(|w| w as u32)
        } else {
            assert!(bits <= 32);

            // Read the two words.
            let w0 = self.read_bits_oneword(16)? as u32;
            let w1 = self.read_bits_oneword(bits - 16)? as u32;

            Ok((w1 << 16) | w0)
        }
    }

    /// Peek from the bitstream, no more than 16 bits (one word).
    fn peek_bits_oneword(&self, bits: u8) -> u16 {
        // Copy paste of `read_bits`, but without advancing the buffer.
        assert!(bits <= 16);

        if bits <= self.remaining {
            self.n.rotate_left(bits as u32) & ((1 << bits) - 1)
        } else {
            let hi = self.n.rotate_left(self.remaining as u32) & ((1 << self.remaining) - 1);
            let bits = bits - self.remaining;

            // We may peek more than we need (i.e. at the end of a chunk), due to the way
            // our decoder is implemented. This is a bit ugly but luckily we can pretend
            // there are just zeros after.
            let n = if self.buffer.is_empty() {
                0
            } else {
                u16::from_le_bytes([self.buffer[0], self.buffer[1]])
            };
            let lo = n.rotate_left(bits as u32) & ((1u32 << bits) as u16).wrapping_sub(1);

            ((hi as u32) << bits) as u16 | lo
        }
    }

    pub fn peek_bits(&self, bits: u8) -> u32 {
        if bits <= 16 {
            self.peek_bits_oneword(bits) as u32
        } else {
            assert!(bits <= 32);

            // Read the two words.
            let lo = self.peek_bits_oneword(16) as u32;
            let hi = self.peek_bits_oneword(bits - 16) as u32;

            (hi << 16) | lo
        }
    }

    pub fn read_u16_le(&mut self) -> Result<u16, DecodeFailed> {
        Ok(self.read_bits_oneword(16)?.swap_bytes())
    }

    pub fn read_u32_le(&mut self) -> Result<u32, DecodeFailed> {
        let lo = self.read_u16_le()? as u32;
        let hi = self.read_u16_le()? as u32;
        Ok((hi << 16) | lo)
    }

    pub fn read_u24_be(&mut self) -> Result<u32, DecodeFailed> {
        let hi = self.read_bits(16)? as u32;
        let lo = self.read_bits(8)? as u32;
        Ok(hi << 8 | lo)
    }

    pub fn align(&mut self) -> bool {
        if self.remaining == 0 {
            false
        } else {
            self.remaining = 0;
            true
        }
    }

    pub fn is_empty(&self) -> bool {
        // > the output bitstream is padded with up to 15 bits of zeros to realign the bitstream
        // > on a 16-bit boundary (even byte boundary) for the next 32 KB of data.
        //
        // TODO but how likely it is to have valid 0 data in the last 15 bits? we would
        // misinterpret this unless we know the decompressed chunk length to know when to
        // stop reading
        self.buffer.is_empty() && self.peek_bits(self.remaining) == 0
    }

    /// Copies from the current buffer to the destination output ignoring the representation.
    ///
    /// The buffer should be aligned beforehand, otherwise bits may be discarded.
    ///
    /// If the output length is not evenly divisible, such padding byte will be discarded.
    pub fn read_raw(&mut self, output: &mut [u8]) -> Result<(), DecodeFailed> {
        // Add 1 to the len if it's odd
        let real_len = output.len() + output.len() % 2;

        if self.buffer.len() < real_len {
            return Err(DecodeFailed::UnexpectedEof);
        }

        output.copy_from_slice(&self.buffer[..output.len()]);
        self.buffer = &self.buffer[real_len..];
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_sequential() {
        // 0..=10 and padding using the least amount of bits possible, read LTR
        let ns = [0b0_1_10_11_100_101_110_1u16, 0b11_1000_1001_1010_00u16];
        let bit_lengths = [1u8, 1, 2, 2, 3, 3, 3, 3, 4, 4, 4];

        // Convert input sequence of 16-bit integers to byte-stream
        let mut bytes = Vec::with_capacity(ns.len() * 2);
        ns.iter().for_each(|n| bytes.extend(&n.to_le_bytes()));

        let mut bitstream = Bitstream::new(&bytes);
        bit_lengths
            .iter()
            .copied()
            .enumerate()
            .for_each(|(value, bit_length)| {
                assert_eq!(bitstream.read_bits(bit_length), Ok(value as u32));
            });
    }

    #[test]
    fn read_16le_aligned() {
        let ns = [0b11100000_00000111_u16, 0b00011111_11111000];
        let mut bytes = Vec::with_capacity(ns.len() * 2);
        ns.iter().for_each(|n| bytes.extend(&n.to_le_bytes()));

        let mut bitstream = Bitstream::new(&bytes);
        assert_eq!(bitstream.read_u16_le(), Ok(0b00000111_11100000));
        assert_eq!(bitstream.read_u16_le(), Ok(0b11111000_00011111));
    }

    #[test]
    fn read_16le_unaligned() {
        let ns = [0b00000000000_10001u16, 0b10000000001_00000];
        let mut bytes = Vec::with_capacity(ns.len() * 2);
        ns.iter().for_each(|n| bytes.extend(&n.to_le_bytes()));

        let mut bitstream = Bitstream::new(&bytes);

        assert_eq!(bitstream.read_bits(11), Ok(0));
        assert_eq!(bitstream.read_u16_le(), Ok(0b00000001_10001_100));
        assert_eq!(bitstream.read_bits(5), Ok(0));
    }

    #[test]
    fn read_32le() {
        let bytes = [0x56, 0x78, 0x12, 0x34];
        let mut bitstream = Bitstream::new(&bytes);

        assert_eq!(bitstream.read_u32_le(), Ok(0x12345678));
    }

    #[test]
    fn read_24be() {
        let ns = [0b0000_1100_0001_1000_u16, 0b0001_1000_0011_0000_u16];
        let mut bytes = Vec::with_capacity(ns.len() * 2);
        ns.iter().for_each(|n| bytes.extend(&n.to_le_bytes()));

        let mut bitstream = Bitstream::new(&bytes);

        assert_eq!(bitstream.read_bits(4), Ok(0));
        assert_eq!(bitstream.read_u24_be(), Ok(0b1100_0001_1000_0001_1000_0011));
        assert_eq!(bitstream.read_bits(4), Ok(0));
    }

    #[test]
    fn align() {
        let bytes = [0b0100_0000, 0b0010_0000, 0b1000_0000, 0b0110_0000];
        let mut bitstream = Bitstream::new(&bytes);

        assert_eq!(bitstream.read_bits(3), Ok(1));
        bitstream.align();
        assert_eq!(bitstream.read_bits(3), Ok(3));
    }

    #[test]
    fn no_remain_after_aligned() {
        let bytes = [0b0100_0000, 0b0010_0000, 0b1000_0000, 0b0110_0000];
        let mut bitstream = Bitstream::new(&bytes);

        bitstream.read_bits(3).unwrap();
        assert_ne!(bitstream.remaining, 0);

        bitstream.align();
        assert_eq!(bitstream.remaining, 0);

        bitstream.read_bits(16).unwrap();
        assert_eq!(bitstream.remaining, 0);
    }

    #[test]
    fn is_empty() {
        let bytes = [];
        let bitstream = Bitstream::new(&bytes);
        assert!(bitstream.is_empty());

        let bytes = [0xab, 0xcd];
        let mut bitstream = Bitstream::new(&bytes);
        assert!(!bitstream.is_empty());
        bitstream.read_bits(15).unwrap();
        assert!(!bitstream.is_empty());
        bitstream.read_bit().unwrap();
        assert!(bitstream.is_empty());
    }

    #[test]
    fn check_read_bit() {
        let bytes = [0b0110_1001, 0b1001_0110];
        let mut bitstream_1 = Bitstream::new(&bytes);
        let mut bitstream_n = Bitstream::new(&bytes);

        (0..16).for_each(|_| {
            assert_eq!(
                bitstream_1.read_bit().map(|b| b as u32),
                bitstream_n.read_bits(1)
            )
        });
    }
}
