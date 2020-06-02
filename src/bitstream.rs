/// > An LZXD bitstream is encoded as a sequence of aligned 16-bit integers stored in the
/// > least-significant- byte to most-significant-byte order, also known as byte-swapped,
/// > or little-endian, words. Given an input stream of bits named a, b, c,..., x, y, z,
/// > A, B, C, D, E, F, the output byte stream MUST be as [ 0| 1| 2| 3|...|30|31].
pub struct Bitstream<'a> {
    buffer: &'a [u8],
    bit_pos: u8,
    output: [u8; 4],
}

impl<'a> Bitstream<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self {
            buffer,
            bit_pos: 0,
            output: [0; 4],
        }
    }

    pub fn read_bit(&mut self) -> u8 {
        let n = u16::from_le_bytes([self.buffer[0], self.buffer[1]]);

        // What the description means is basically that we read the bits left-to-right
        // (similar to how they would be written out in code), using the MSB first.
        let bit = ((n >> (15 - self.bit_pos)) & 1) as u8;

        // The way we advance in the buffer of 16-bit integer is by advancing 2 bytes as
        // soon as the bit position wraps around the next 16-bit integer (modulo 16).
        self.bit_pos = if self.bit_pos == 15 {
            self.buffer = &self.buffer[2..];
            0
        } else {
            self.bit_pos + 1
        };

        bit
    }

    pub fn read_bits(&mut self, bits: u8) -> &[u8] {
        let bits = bits as usize;
        assert!(bits <= self.output.len() * 8);

        self.output.iter_mut().for_each(|x| *x = 0);
        (0..bits).for_each(|i| {
            self.output[i / 8] = (self.output[i / 8] << 1) | self.read_bit();
        });
        &self.output[..(bits + 7) / 8]
    }

    pub fn read_u16_le(&mut self) -> u16 {
        let buffer = self.read_bits(16);
        u16::from_le_bytes([buffer[0], buffer[1]])
    }

    pub fn read_u24_be(&mut self) -> u32 {
        let buffer = self.read_bits(24);
        u32::from_be_bytes([0, buffer[0], buffer[1], buffer[2]])
    }
}
