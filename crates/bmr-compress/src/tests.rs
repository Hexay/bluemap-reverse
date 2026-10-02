use std::io::Write;

use flate2::Compression;
use flate2::write::{GzEncoder, ZlibEncoder};

use super::*;

/// What a hires tile starts with (PRBM format version) followed by filler.
const TILE: &[u8] = b"\x01\x07prbm tile prbm tile prbm tile prbm tile";
const LIMIT: usize = 1 << 20;

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

fn zstd(data: &[u8]) -> Vec<u8> {
    ruzstd::encoding::compress_to_vec(data, ruzstd::encoding::CompressionLevel::Fastest)
}

#[test]
fn every_format_is_sniffed_and_decoded() {
    let cases = [
        (gzip(TILE), Format::Gzip),
        (zlib(TILE), Format::Zlib),
        (zstd(TILE), Format::Zstd),
        (lz4_block::compress(TILE), Format::Lz4Block),
    ];
    for (bytes, format) in cases {
        assert_eq!(Format::sniff(&bytes), Some(format));
        assert_eq!(decompress_any(bytes, LIMIT).unwrap(), TILE, "{format:?}");
    }
}

#[test]
fn uncompressed_bodies_pass_through() {
    for body in [TILE, b"{\"maps\":[]}", b"[]", b"\x89PNG\r\n\x1a\n"] {
        assert_eq!(Format::sniff(body), None);
        assert_eq!(decompress_any(body.to_vec(), LIMIT).unwrap(), body);
    }
}

#[test]
fn output_over_limit_is_refused() {
    let big = vec![0; 10_000];
    for bytes in [gzip(&big), zlib(&big), zstd(&big), lz4_block::compress(&big)] {
        let e = decompress_any(bytes, 4096).unwrap_err();
        assert!(format!("{e:#}").contains("over 4096 bytes"), "{e:#}");
    }
}

#[test]
fn corrupt_input_is_an_error() {
    for mut bytes in [gzip(TILE), zlib(TILE), zstd(TILE)] {
        bytes.truncate(bytes.len() / 2);
        assert!(decompress_any(bytes, LIMIT).is_err());
    }
}
