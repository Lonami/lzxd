use std::{
    convert::TryInto,
    path::{Path, PathBuf},
};

use lzxd::{Lzxd, WindowSize};

struct Test {
    input: PathBuf,
    output: PathBuf,
}

fn discover_tests() -> Vec<Test> {
    let testdata_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");

    // Scan for all tests in testdata/
    let mut tests = Vec::new();
    for entry in std::fs::read_dir(&testdata_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        // All tests start with .lzx and have a corresponding .out file.
        match path.extension().map(|s| s.to_str()).flatten() {
            Some("lzx") => {}
            _ => continue,
        }

        if path.is_file() {
            // Ensure the corresponding .out file exists.
            let out_path = path.with_extension("out");
            if !out_path.exists() {
                panic!("Missing output file for test: {:?}", out_path);
            }

            tests.push(Test {
                input: path,
                output: out_path,
            });
        }
    }

    tests
}

fn run_testdata(mut data: impl std::io::Read, mut expected: impl std::io::Read) {
    let mut buf = [0u8; 8];

    // Read file header.
    data.read_exact(&mut buf[..4]).unwrap();
    let ws = u32::from_le_bytes(buf[..4].try_into().unwrap());
    data.read_exact(&mut buf[..4]).unwrap(); // Discard.

    let ws = match ws {
        0x0000_8000 => WindowSize::KB32,
        0x0001_0000 => WindowSize::KB64,
        0x0002_0000 => WindowSize::KB128,
        0x0004_0000 => WindowSize::KB256,
        0x0008_0000 => WindowSize::KB512,
        0x0010_0000 => WindowSize::MB1,
        0x0020_0000 => WindowSize::MB2,
        0x0040_0000 => WindowSize::MB4,
        0x0080_0000 => WindowSize::MB8,
        0x0100_0000 => WindowSize::MB16,
        0x0200_0000 => WindowSize::MB32,
        _ => panic!("invalid window size"),
    };

    let mut lzxd = Lzxd::new(ws);
    let mut chunk = Vec::new();
    let mut expected_output = Vec::new();

    loop {
        match data.read(&mut buf[..8]) {
            // Check for the end of the stream.
            Ok(n) if n < 8 => break,
            Err(_) => break,

            Ok(_) => {}
        }

        let chunk_len = usize::from_le_bytes(buf.try_into().unwrap());
        data.read_exact(&mut buf[..8]).unwrap();
        let output_len = usize::from_le_bytes(buf.try_into().unwrap());

        chunk.resize(chunk_len, 0);
        expected_output.resize(output_len, 0);

        data.read_exact(&mut chunk).unwrap();
        expected.read_exact(&mut expected_output).unwrap();
        let res = lzxd.decompress_next(&mut chunk, output_len).unwrap();

        assert_eq!(res, expected_output);
    }
}

fn run_test(test: &Test) {
    let inp = std::fs::File::open(&test.input).unwrap();
    let out = std::fs::File::open(&test.output).unwrap();

    eprintln!("Testing: {}", test.input.display());
    run_testdata(inp, out);
}

#[test]
fn decompression_tests() {
    let tests = discover_tests();
    for test in tests {
        run_test(&test);
    }
}
