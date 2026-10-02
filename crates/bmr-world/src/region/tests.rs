use std::path::PathBuf;

use flate2::write::GzEncoder;

use super::*;

/// Unique per test so parallel tests never share a file; removed on drop.
struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        Self(std::env::temp_dir().join(format!("bmr-region-{}-{name}.mca", std::process::id())))
    }

    fn with(name: &str, bytes: &[u8]) -> Self {
        let f = Self::new(name);
        std::fs::write(&f.0, bytes).unwrap();
        f
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Region with one chunk in slot (lx, lz) at sector 2: `len` field, compression byte, payload.
fn region_with(slot: (u8, u8), len: u32, kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut data = vec![0u8; 2 * SECTOR];
    let i = slot.1 as usize * 32 + slot.0 as usize;
    data[i * 4..i * 4 + 4].copy_from_slice(&((2u32 << 8) | 1).to_be_bytes());
    data.extend(len.to_be_bytes());
    data.push(kind);
    data.extend(payload);
    data.resize(data.len().next_multiple_of(SECTOR), 0);
    data
}

fn read_all(name: &str, bytes: &[u8]) -> Result<RegionChunks> {
    let f = TempFile::with(name, bytes);
    read_region_where(&f.0, &|_| true)
}

fn err_of(name: &str, bytes: &[u8]) -> String {
    format!("{:#}", read_all(name, bytes).expect_err("expected an error"))
}

#[test]
fn write_then_read_keeps_slots_and_payloads() {
    let f = TempFile::new("roundtrip");
    let chunks: RegionChunks = vec![((0, 0), b"first".to_vec()), ((31, 0), vec![7; 9000]), ((5, 31), Vec::new())];
    write_region(&f.0, &chunks).unwrap();
    let mut back = read_region_where(&f.0, &|_| true).unwrap();
    back.sort();
    let mut want = chunks.clone();
    want.sort();
    assert_eq!(back, want);
    assert_eq!(std::fs::metadata(&f.0).unwrap().len() % SECTOR as u64, 0);

    let only = read_region_where(&f.0, &|(x, _)| x == 31).unwrap();
    assert_eq!(only, vec![((31, 0), vec![7; 9000])]);
}

#[test]
fn gzip_and_uncompressed_chunks_read() {
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(b"gzipped").unwrap();
    let gz = gz.finish().unwrap();
    let got = read_all("gzip", &region_with((3, 4), gz.len() as u32 + 1, 1, &gz)).unwrap();
    assert_eq!(got, vec![((3, 4), b"gzipped".to_vec())]);
    let got = read_all("raw", &region_with((0, 0), 4, 3, b"raw")).unwrap();
    assert_eq!(got, vec![((0, 0), b"raw".to_vec())]);
}

#[test]
fn empty_file_is_an_empty_region() {
    assert!(read_all("empty", &[]).unwrap().is_empty());
}

#[test]
fn truncated_header_is_an_error() {
    assert!(err_of("truncated", &[0u8; SECTOR + 10]).contains("truncated header"));
}

#[test]
fn chunk_offset_past_eof_is_an_error() {
    let mut data = vec![0u8; 2 * SECTOR];
    data[..4].copy_from_slice(&((9u32 << 8) | 1).to_be_bytes());
    let e = err_of("offset", &data);
    assert!(e.contains("slot 0") && e.contains("offset past end"), "{e}");
}

#[test]
fn bad_chunk_lengths_are_errors() {
    assert!(err_of("len0", &region_with((0, 0), 0, 2, b"")).contains("bad chunk length 0"));
    let e = err_of("len_long", &region_with((0, 0), 5000, 2, b"x"));
    assert!(e.contains("bad chunk length 5000"), "{e}");
}

#[test]
fn external_mcc_chunk_is_an_error() {
    let e = err_of("mcc", &region_with((0, 0), 1, 0x82, b""));
    assert!(e.contains(".mcc"), "{e}");
}

/// One lz4-java block holding `data`; level nibble 6 = 64 KiB blocks, lz4-java's default.
fn lz4_block(method: u8, data: &[u8]) -> Vec<u8> {
    let payload = if method == lz4::LZ4 { lz4_flex::block::compress(data) } else { data.to_vec() };
    let mut b = lz4::MAGIC.to_vec();
    b.push(method | 6);
    for v in [payload.len() as u32, data.len() as u32, lz4::checksum(data)] {
        b.extend(v.to_le_bytes());
    }
    b.extend(payload);
    b
}

fn lz4_end() -> Vec<u8> {
    let mut b = lz4::MAGIC.to_vec();
    b.push(lz4::RAW | 6);
    b.extend([0; 12]);
    b
}

fn lz4_region(stream: &[u8]) -> Vec<u8> {
    region_with((1, 2), stream.len() as u32 + 1, 4, stream)
}

/// 150 KB over three 64 KiB blocks: LZ4, raw (lz4-java's choice for incompressible blocks), LZ4.
fn lz4_sample() -> (Vec<u8>, Vec<u8>) {
    let nbt: Vec<u8> = (0..150_000u32).map(|i| (i / 3 % 251) as u8).collect();
    let mut stream = lz4_block(lz4::LZ4, &nbt[..65536]);
    stream.extend(lz4_block(lz4::RAW, &nbt[65536..131072]));
    stream.extend(lz4_block(lz4::LZ4, &nbt[131072..]));
    (nbt, stream)
}

fn set_le(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

#[test]
fn lz4_java_chunks_read() {
    let (nbt, mut stream) = lz4_sample();
    assert!(stream.len() < nbt.len() - 50_000, "LZ4 blocks should compress");
    let unterminated = read_all("lz4_noend", &lz4_region(&stream)).unwrap();
    stream.extend(lz4_end());
    let got = read_all("lz4", &lz4_region(&stream)).unwrap();
    assert_eq!(got, vec![((1, 2), nbt)]);
    assert_eq!(unterminated, got);
}

#[test]
fn corrupt_lz4_streams_are_errors() {
    let (_, stream) = lz4_sample();
    type Case = (&'static str, fn(&mut Vec<u8>), &'static str);
    let cases: [Case; 7] = [
        ("magic", |s| s[3] = b'X', "bad block magic"),
        ("header", |s| s.truncate(lz4::HEADER - 1), "truncated block header"),
        ("payload", |s| s.truncate(s.len() - 10), "truncated block payload"),
        ("checksum", |s| s[17] ^= 1, "checksum mismatch"),
        ("method", |s| s[8] = 0x30 | 6, "unknown block method"),
        ("over_level", |s| set_le(s, 13, 65537), "bad block lengths"),
        ("over_ratio", |s| set_le(s, 9, 65536 / 256 - 1), "bad block lengths"),
    ];
    for (name, corrupt, want) in cases {
        let mut s = stream.clone();
        corrupt(&mut s);
        let e = err_of(&format!("lz4_{name}"), &lz4_region(&s));
        assert!(e.contains(want), "{name}: {e}");
    }
}

#[test]
fn lz4_declared_length_mismatches_are_errors() {
    let mut s = lz4_block(lz4::RAW, b"raw block");
    set_le(&mut s, 13, 8);
    assert!(err_of("lz4_rawlen", &lz4_region(&s)).contains("bad block lengths 9/8"));

    let mut s = lz4_block(lz4::LZ4, &[1; 1000]);
    set_le(&mut s, 13, 1001);
    let e = err_of("lz4_long", &lz4_region(&s));
    assert!(e.contains("decompressed to 1000 bytes, header says 1001"), "{e}");
    set_le(&mut s, 13, 999);
    assert!(err_of("lz4_short", &lz4_region(&s)).contains("lz4: "));
}

#[test]
fn unknown_compression_is_a_clear_error() {
    for kind in [5, 127] {
        let e = err_of(&format!("kind{kind}"), &region_with((0, 0), 4, kind, b"abc"));
        assert!(e.contains(&format!("unsupported chunk compression {kind}")), "{e}");
    }
}

#[test]
fn corrupt_zlib_is_an_error() {
    assert!(read_all("zlib", &region_with((0, 0), 5, 2, b"junk")).is_err());
}

#[test]
fn oversized_chunk_is_refused_on_write() {
    // xorshift noise: incompressible, so 1.1 MiB stays over the 255-sector limit
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    let noise: Vec<u8> = (0..1_100_000)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    let f = TempFile::new("oversized");
    let e = write_region(&f.0, &[((1, 2), noise)]).expect_err("expected an error");
    assert!(e.to_string().contains("chunk 1,2 too large"), "{e}");
}
