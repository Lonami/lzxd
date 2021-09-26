#![no_main]
use libfuzzer_sys::fuzz_target;
use lzxd::{Lzxd, WindowSize};

fuzz_target!(|data: &[u8]| {
    const WINDOW_SIZES: &[WindowSize] = &[
        WindowSize::KB32,
        WindowSize::KB64,
        WindowSize::KB128,
        WindowSize::KB256,
        WindowSize::KB512,
        WindowSize::MB1,
        WindowSize::MB2,
        WindowSize::MB4,
        WindowSize::MB8,
        WindowSize::MB16,
        WindowSize::MB32,
    ];

    for ws in WINDOW_SIZES {
        let mut lzxd = Lzxd::new(*ws);
        let _ = lzxd.decompress_next(data);
    }
});
