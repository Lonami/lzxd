use crate::{Bitstream, BlockHead, BlockType, CanonicalTree, Tree, WindowSize};
use std::convert::TryFrom;

// if position_slot < 4 {
//     0
// } else if position_slot >= 36 {
//     17
// } else {
//     (position_slot - 2) / 2
// }
const FOOTER_BITS: [u8; 289] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13, 14, 14, 15, 15, 16, 16, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
];

// if position_slot == 0 {
//     0
// } else {
//     BASE_POSITION[position_slot - 1] + (1 << FOOTER_BITS[position_slot - 1])
// }
const BASE_POSITION: [u32; 290] = [
    0, 1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536,
    2048, 3072, 4096, 6144, 8192, 12288, 16384, 24576, 32768, 49152, 65536, 98304, 131072, 196608,
    262144, 393216, 524288, 655360, 786432, 917504, 1048576, 1179648, 1310720, 1441792, 1572864,
    1703936, 1835008, 1966080, 2097152, 2228224, 2359296, 2490368, 2621440, 2752512, 2883584,
    3014656, 3145728, 3276800, 3407872, 3538944, 3670016, 3801088, 3932160, 4063232, 4194304,
    4325376, 4456448, 4587520, 4718592, 4849664, 4980736, 5111808, 5242880, 5373952, 5505024,
    5636096, 5767168, 5898240, 6029312, 6160384, 6291456, 6422528, 6553600, 6684672, 6815744,
    6946816, 7077888, 7208960, 7340032, 7471104, 7602176, 7733248, 7864320, 7995392, 8126464,
    8257536, 8388608, 8519680, 8650752, 8781824, 8912896, 9043968, 9175040, 9306112, 9437184,
    9568256, 9699328, 9830400, 9961472, 10092544, 10223616, 10354688, 10485760, 10616832, 10747904,
    10878976, 11010048, 11141120, 11272192, 11403264, 11534336, 11665408, 11796480, 11927552,
    12058624, 12189696, 12320768, 12451840, 12582912, 12713984, 12845056, 12976128, 13107200,
    13238272, 13369344, 13500416, 13631488, 13762560, 13893632, 14024704, 14155776, 14286848,
    14417920, 14548992, 14680064, 14811136, 14942208, 15073280, 15204352, 15335424, 15466496,
    15597568, 15728640, 15859712, 15990784, 16121856, 16252928, 16384000, 16515072, 16646144,
    16777216, 16908288, 17039360, 17170432, 17301504, 17432576, 17563648, 17694720, 17825792,
    17956864, 18087936, 18219008, 18350080, 18481152, 18612224, 18743296, 18874368, 19005440,
    19136512, 19267584, 19398656, 19529728, 19660800, 19791872, 19922944, 20054016, 20185088,
    20316160, 20447232, 20578304, 20709376, 20840448, 20971520, 21102592, 21233664, 21364736,
    21495808, 21626880, 21757952, 21889024, 22020096, 22151168, 22282240, 22413312, 22544384,
    22675456, 22806528, 22937600, 23068672, 23199744, 23330816, 23461888, 23592960, 23724032,
    23855104, 23986176, 24117248, 24248320, 24379392, 24510464, 24641536, 24772608, 24903680,
    25034752, 25165824, 25296896, 25427968, 25559040, 25690112, 25821184, 25952256, 26083328,
    26214400, 26345472, 26476544, 26607616, 26738688, 26869760, 27000832, 27131904, 27262976,
    27394048, 27525120, 27656192, 27787264, 27918336, 28049408, 28180480, 28311552, 28442624,
    28573696, 28704768, 28835840, 28966912, 29097984, 29229056, 29360128, 29491200, 29622272,
    29753344, 29884416, 30015488, 30146560, 30277632, 30408704, 30539776, 30670848, 30801920,
    30932992, 31064064, 31195136, 31326208, 31457280, 31588352, 31719424, 31850496, 31981568,
    32112640, 32243712, 32374784, 32505856, 32636928, 32768000, 32899072, 33030144, 33161216,
    33292288, 33423360,
];

/// > A chunk represents exactly 32 KB of uncompressed data until the last chunk in the stream,
/// > which can represent less than 32 KB.
const MAX_CHUNK_SIZE: usize = 32 * 1024;

pub struct Lzxd<'a> {
    /// The window size we're working with.
    window_size: WindowSize,

    /// Sliding window into which data is decompressed.
    // TODO proper `Window` struct that handles wrap around for us.
    window: Vec<u8>,

    /// Current position into the sliding window.
    pos: usize,

    /// Bitstream over the in-memory byte buffer of compressed data.
    bitstream: Bitstream<'a>,

    /// This tree cannot be used directly, it exists only to apply the delta of upcoming trees
    /// to its path lengths.
    main_tree: CanonicalTree,

    /// This tree cannot be used directly, it exists only to apply the delta of upcoming trees
    /// to its path lengths.
    length_tree: CanonicalTree,

    /// > The three most recent real match offsets are kept in a list.
    r: [u32; 3],

    /// Has the very first chunk been read yet? Unlike the rest, it has additional data.
    first_chunk_read: bool,

    /// This field will update after the first chunk is read, but will remain being `None`
    /// if the E8 Call Translation is not enabled for this stream.
    _e8_translation_size: Option<u32>,

    /// Current block.
    current_block: BlockHead,

    /// The remaining size we can read from `current_block` before we need a new one.
    block_remaining: u32,
}

impl<'a> Lzxd<'a> {
    /// NOTE: If the `WindowSize` is wrong, things won't work as expected.
    pub fn new(window_size: WindowSize, buffer: &'a [u8]) -> Self {
        // > The main tree comprises 256 elements that correspond to all possible 8-bit
        // > characters, plus 8 * NUM_POSITION_SLOTS elements that correspond to matches.
        let main_tree = CanonicalTree::new(256 + 8 * window_size.position_slots());

        // > The length tree comprises 249 elements.
        let length_tree = CanonicalTree::new(249);

        Self {
            window_size,
            window: window_size.create_buffer(),
            pos: 0,
            bitstream: Bitstream::new(buffer),
            // > Because trees are output several times during compression of large amounts of
            // > data (multiple blocks), LZXD optimizes compression by encoding only the delta
            // > path lengths lengths between the current and previous trees.
            //
            // Because it uses deltas, we need to store the previous value across blocks.
            main_tree,
            length_tree,
            // > The initial state of R0, R1, R2 is (1, 1, 1).
            r: [1, 1, 1],
            first_chunk_read: false,
            _e8_translation_size: None,
            // Start with some dummy value.
            current_block: BlockHead::Uncompressed {
                size: 0,
                r: [1, 1, 1],
            },
            block_remaining: 0,
        }
    }

    /// Skips a chunk without actually reading anything from it.
    pub fn skip_chunk(&mut self) {
        todo!()
    }

    /// Reads the header for the next chunk and returns the chunk size.
    pub fn read_chunk_header(&mut self) {
        // > The first bit in the first chunk in the LZXD bitstream (following the 2-byte,
        // > chunk-size prefix described in section 2.2.1) indicates the presence or absence of
        // > two 16-bit fields immediately following the single bit. If the bit is set, E8
        // > translation is enabled.
        if !self.first_chunk_read {
            self.first_chunk_read = true;

            let e8_translation = self.bitstream.read_bit() != 0;
            self._e8_translation_size = if e8_translation {
                let high = self.bitstream.read_u16_le() as u32;
                let low = self.bitstream.read_u16_le() as u32;
                Some((high << 16) | low)
            } else {
                None
            };

            // We don't support e8 translation yet
            if self._e8_translation_size.is_some() {
                todo!("e8 translation not implemented");
            }
        }
    }

    /// Read the pretrees for the main and length tree, and with those also read the trees
    /// themselves, using the path lengths from a previous tree if any.
    ///
    /// This is used when reading a verbatim or aligned block.
    fn read_main_and_length_trees(&mut self) {
        // Verbatim block
        // Entry                                             Comments
        // Pretree for first 256 elements of main tree       20 elements, 4 bits each
        // Path lengths of first 256 elements of main tree   Encoded using pretree
        // Pretree for remainder of main tree                20 elements, 4 bits each
        // Path lengths of remaining elements of main tree   Encoded using pretree
        // Pretree for length tree                           20 elements, 4 bits each
        // Path lengths of elements in length tree           Encoded using pretree
        // Token sequence (matches and literals)             Specified in section 2.6
        self.main_tree
            .update_range_with_pretree(&mut self.bitstream, 0..256);

        self.main_tree.update_range_with_pretree(
            &mut self.bitstream,
            256..256 + 8 * self.window_size.position_slots(),
        );
        self.length_tree
            .update_range_with_pretree(&mut self.bitstream, 0..249);
    }

    /// Read the head of the next block and store it in the `self.current_block`.
    fn read_block_head(&mut self) {
        // Block header
        let ty = match BlockType::try_from(self.bitstream.read_bits(3) as u8) {
            Ok(ty) => ty,
            Err(_) => todo!("notify error of bad block type"),
        };
        let size = self.bitstream.read_u24_be();

        // Block body (head)
        self.block_remaining = size;
        self.current_block = match ty {
            BlockType::Verbatim => {
                self.read_main_and_length_trees();

                BlockHead::Verbatim {
                    size,
                    main_tree: self.main_tree.create_instance(),
                    length_tree: self.length_tree.create_instance(),
                }
            }
            BlockType::AlignedOffset => {
                // > An aligned offset block is identical to the verbatim block except for the
                // > presence of the aligned offset tree preceding the other trees.
                let aligned_offset_tree = {
                    let mut path_lengths = vec![0u8; 8];
                    path_lengths
                        .iter_mut()
                        .for_each(|x| *x = self.bitstream.read_bits(3) as u8);

                    Tree::from_path_lengths(path_lengths)
                };

                self.read_main_and_length_trees();

                BlockHead::AlignedOffset {
                    size,
                    aligned_offset_tree,
                    main_tree: self.main_tree.create_instance(),
                    length_tree: self.length_tree.create_instance(),
                }
            }
            BlockType::Uncompressed => {
                self.r = [
                    self.bitstream.read_u32_le(),
                    self.bitstream.read_u32_le(),
                    self.bitstream.read_u32_le(),
                ];
                BlockHead::Uncompressed {
                    size,
                    r: self.r.clone(),
                }
            }
        };
    }

    pub fn next_chunk(&mut self, chunk_size: usize) -> Option<&[u8]> {
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
        // TODO instead of panicking, we should probably return proper errors (here and everywhere)
        assert!(chunk_size as usize <= MAX_CHUNK_SIZE);

        self.read_chunk_header();

        if self.block_remaining == 0 {
            self.read_block_head();
        }

        // Both verbatim and aligned offset block need to decode matches and literals, so their
        // code path is mostly shared. However, uncompressed blocks are so different that they
        // get their own code path.
        let aligned_offset_tree;
        let main_tree;
        let length_tree;
        match &self.current_block {
            BlockHead::Verbatim {
                main_tree: main,
                length_tree: length,
                ..
            } => {
                aligned_offset_tree = None;
                main_tree = main;
                length_tree = length;
            }
            BlockHead::AlignedOffset {
                aligned_offset_tree: aligned,
                main_tree: main,
                length_tree: length,
                ..
            } => {
                aligned_offset_tree = Some(aligned);
                main_tree = main;
                length_tree = length;
            }
            BlockHead::Uncompressed { size: _size, r: _r } => todo!(),
        }

        // This is the code path for aligned and verbatim blocks.
        while self.block_remaining != 0 {
            let mut curpos = self.pos;
            let limit = usize::min(chunk_size as usize, self.block_remaining as usize);
            let end = curpos + limit;
            while curpos < end {
                // Decoding Matches and Literals (Aligned and Verbatim Blocks)
                let main_element = main_tree.decode_element(&mut self.bitstream);

                // Check if it is a literal character.
                if main_element < 256 {
                    // It is a literal, so copy the literal to output.
                    self.window[curpos] = main_element as u8;
                    curpos += 1;
                } else {
                    // Decode the match. For a match, there are two components, offset and length.
                    let length_header = (main_element - 256) & 7;

                    let match_length = if length_header == 7 {
                        // Length of the footer.
                        length_tree.decode_element(&mut self.bitstream) + 7 + 2
                    } else {
                        length_header + 2 // no length footer
                                          // Decoding a match length (if a match length < 257).
                    };

                    let position_slot = (main_element - 256) >> 3;

                    // Check for repeated offsets (positions 0, 1, 2).
                    let match_offset;
                    if position_slot == 0 {
                        match_offset = self.r[0];
                    } else if position_slot == 1 {
                        match_offset = self.r[1];
                        self.r.swap(0, 1);
                    } else if position_slot == 2 {
                        match_offset = self.r[2];
                        self.r.swap(0, 2);
                    } else {
                        // Not a repeated offset.
                        let offset_bits = FOOTER_BITS[position_slot as usize];

                        let formatted_offset = if let Some(aligned_offset_tree) =
                            aligned_offset_tree.as_ref()
                        {
                            let verbatim_bits;
                            let aligned_bits;

                            // This means there are some aligned bits.
                            if offset_bits >= 3 {
                                verbatim_bits = (self.bitstream.read_bits(offset_bits - 3)) << 3;
                                aligned_bits =
                                    aligned_offset_tree.decode_element(&mut self.bitstream);
                            } else {
                                // 0, 1, or 2 verbatim bits
                                verbatim_bits = self.bitstream.read_bits(offset_bits);
                                aligned_bits = 0;
                            }

                            BASE_POSITION[position_slot as usize]
                                + verbatim_bits as u32
                                + aligned_bits as u32
                        } else {
                            // Block_type is a verbatim_block.
                            let verbatim_bits = self.bitstream.read_bits(offset_bits);
                            BASE_POSITION[position_slot as usize] + verbatim_bits as u32
                        };

                        // Decoding a match offset.
                        match_offset = formatted_offset - 2;

                        // Update repeated offset least recently used queue.
                        self.r[2] = self.r[1];
                        self.r[1] = self.r[0];
                        self.r[0] = match_offset;
                    }

                    // Check for extra length.
                    // > If the match length is 257 or larger, the encoded match length token
                    // > (or match length, as specified in section 2.6) value is 257, and an
                    // > encoded Extra Length field follows the other match encoding components,
                    // > as specified in section 2.6.7, in the bitstream.

                    // TODO for some reason, if we do this, parsing .xnb files with window size
                    //      64KB, it breaks and stops decompressing correctly, but no idea why.
                    /*
                    let match_length = if match_length == 257 {
                        // Decode the extra length.
                        let extra_len = if self.bitstream.read_bit() != 0 {
                            if self.bitstream.read_bit() != 0 {
                                if self.bitstream.read_bit() != 0 {
                                    // > Prefix 0b111; Number of bits to decode 15;
                                    self.bitstream.read_bits(15)
                                } else {
                                    // > Prefix 0b110; Number of bits to decode 12;
                                    self.bitstream.read_bits(12) + 1024 + 256
                                }
                            } else {
                                // > Prefix 0b10; Number of bits to decode 10;
                                self.bitstream.read_bits(10) + 256
                            }
                        } else {
                            // > Prefix 0b0; Number of bits to decode 8;
                            self.bitstream.read_bits(8)
                        };

                        // Get the match length (if match length >= 257).
                        // In all cases,
                        // > Base value to add to decoded value 257 + …
                        257 + extra_len
                    } else {
                        match_length as u16
                    };
                    */

                    let match_offset = match_offset as usize;
                    let match_length = match_length as usize;

                    // Get match length and offset. Perform copy and paste work.
                    // TODO this can be improved by avoiding %
                    for i in 0..match_length {
                        let li = (curpos + i) % self.window.len();
                        let ri =
                            (self.window.len() + curpos + i - match_offset) % self.window.len();
                        self.window[li] = self.window[ri];
                    }

                    // TODO something is still wrong around here, i don't know what it is
                    //      guess add more debug logs and try to find at which point it breaks

                    curpos += match_length;
                }
            }
            self.block_remaining -= limit as u32;

            // > To ensure that an exact number of input bytes represent an exact number of
            // > output bytes for each chunk, after each 32 KB of uncompressed data is
            // > represented in the output compressed bitstream, the output bitstream is padded
            // > with up to 15 bits of zeros to realign the bitstream on a 16-bit boundary
            // > (even byte boundary) for the next 32 KB of data. This results in a compressed
            // > chunk of a byte-aligned size. The compressed chunk could be smaller than 32 KB
            // > or larger than 32 KB if the data is incompressible when the chunk is not the
            // > last one.
            self.bitstream.align();

            let start = self.pos;
            self.pos = curpos % self.window.len();
            return Some(&self.window[start..end]);
        }

        todo!()
    }
}
