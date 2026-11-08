#![no_main]

use bmr_compress::Format;
use libfuzzer_sys::fuzz_target;

const LIMIT: usize = 1 << 20;

// first byte picks a decoder directly, so the fuzzer need not rediscover each magic
fuzz_target!(|data: &[u8]| {
    let Some((&pick, rest)) = data.split_first() else { return };
    let format = [Format::Gzip, Format::Zlib, Format::Zstd, Format::Lz4Block][pick as usize % 4];
    if let Ok(out) = format.decompress(rest, LIMIT) {
        assert!(out.len() <= LIMIT);
    }
    if let Ok(out) = bmr_compress::decompress_any(rest.to_vec(), LIMIT) {
        assert!(out.len() <= LIMIT.max(rest.len()));
    }
});
