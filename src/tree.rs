use crate::Bitstream;
use std::ops::Range;

pub struct Tree {
    // > Each tree element can have a path length of [0, 16], where a zero path length indicates
    // > that the element has a zero frequency and is not present in the tree.
    //
    // We represent them as `u8` due to their very short range.
    _path_lengths: Vec<u8>,
    huffman_tree: Vec<u16>,
}

impl Tree {
    pub fn new(count: usize) -> Self {
        Self {
            // > In the case of the very first such tree, the delta is calculated against a tree
            // > in which all elements have a zero path length.
            _path_lengths: vec![0; count],
            huffman_tree: vec![],
        }
    }

    pub fn from_path_lengths(path_lengths: Vec<u8>) -> Self {
        let mut tree = Self {
            _path_lengths: path_lengths,
            huffman_tree: vec![],
        };
        tree.decode_lengths();
        tree
    }

    pub fn decode_lengths(&mut self) {
        todo!("decode path lengths into a huffman tree")
    }

    pub fn decode_element(&self, _bitstream: &mut Bitstream) -> u16 {
        todo!("decode element from huffman tree")
    }

    // Note: the tree already exists and is used to apply the deltas.
    pub fn update_range_with_pretree(&mut self, bitstream: &mut Bitstream, range: Range<usize>) {
        // > Each of the 17 possible values of (len[x] - prev_len[x]) mod 17, plus three
        // > additional codes used for run-length encoding, are not output directly as 5-bit
        // > numbers but are instead encoded via a Huffman tree called the pretree. The pretree
        // > is generated dynamically according to the frequencies of the 20 allowable tree
        // > codes. The structure of the pretree is encoded in a total of 80 bits by using 4 bits
        // > to output the path length of each of the 20 pretree elements. Once again, a zero
        // > path length indicates a zero-frequency element.
        let pretree = {
            let mut path_lengths = vec![0u8; 20];
            path_lengths
                .iter_mut()
                .for_each(|x| *x = bitstream.read_bits(4) as u8);

            Tree::from_path_lengths(path_lengths)
        };

        // > Tree elements are output in sequential order starting with the first element.
        let mut i = range.start;
        while i < range.end {
            // > The "real" tree is then encoded using the pretree Huffman codes.
            let code = pretree.decode_element(bitstream);

            // > Elements can be encoded in one of two ways: if several consecutive elements have
            // > the same path length, run-length encoding is employed; otherwise, the element is
            // > output by encoding the difference between the current path length and the
            // > previous path length of the tree, mod 17.
            match code {
                0..=16 => {
                    self.huffman_tree[i] = (17 + self.huffman_tree[i] - code) % 17;
                    i += 1;
                }
                // > Codes 17, 18, and 19 are used to represent consecutive elements that have the
                // > same path length.
                17 => {
                    let zeros = bitstream.read_bits(4);
                    self.huffman_tree[i..i + zeros as usize + 4]
                        .iter_mut()
                        .for_each(|x| *x = 0);
                    i += zeros as usize + 4;
                }
                18 => {
                    let zeros = bitstream.read_bits(5);
                    self.huffman_tree[i..i + zeros as usize + 20]
                        .iter_mut()
                        .for_each(|x| *x = 0);
                    i += zeros as usize + 20;
                }
                19 => {
                    let same = bitstream.read_bits(1);
                    // "Decode new code" is used to parse the next code from the bitstream, which
                    // has a value range of [0, 16].
                    let code = pretree.decode_element(bitstream);
                    let value = (17 + self.huffman_tree[i] - code) % 17;
                    self.huffman_tree[i..i + same as usize + 4]
                        .iter_mut()
                        .for_each(|x| *x = value);
                    i += same as usize + 4;
                }
                _ => panic!(format!("invalid pretree code element {}", code)),
            };
        }
    }
}
