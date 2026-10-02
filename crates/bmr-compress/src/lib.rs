//! Decompression shared by the tile fetcher (BlueMap storage compressions, docs/research/01 §1) and the region
//! reader (chunk compression types). Every decoder refuses output over `limit` bytes.

pub mod lz4_block;

use std::io::Read;

use anyhow::{Context, Result, anyhow, ensure};
use flate2::read::{GzDecoder, ZlibDecoder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Gzip,
    /// BlueMap's `deflate` (Java's `DeflaterOutputStream`) and chunk compression 2.
    Zlib,
    Zstd,
    /// BlueMap's `lz4` and chunk compression 4.
    Lz4Block,
}

impl Format {
    /// By magic bytes; `None` = not compressed (PRBM, JSON and PNG start differently).
    pub fn sniff(data: &[u8]) -> Option<Self> {
        match data {
            [0x1f, 0x8b, ..] => Some(Self::Gzip),
            [0x28, 0xb5, 0x2f, 0xfd, ..] => Some(Self::Zstd),
            [b0 @ 0x78, b1, ..] if (u16::from(*b0) << 8 | u16::from(*b1)) % 31 == 0 => Some(Self::Zlib),
            _ if data.starts_with(lz4_block::MAGIC) => Some(Self::Lz4Block),
            _ => None,
        }
    }

    pub fn decompress(self, data: &[u8], limit: usize) -> Result<Vec<u8>> {
        match self {
            Self::Gzip => read_capped(GzDecoder::new(data), limit).context("gzip"),
            Self::Zlib => read_capped(ZlibDecoder::new(data), limit).context("zlib"),
            Self::Zstd => {
                let decoder = ruzstd::decoding::StreamingDecoder::new(data).map_err(|e| anyhow!("zstd: {e}"))?;
                read_capped(decoder, limit).context("zstd")
            }
            Self::Lz4Block => lz4_block::decompress(data, limit),
        }
    }
}

/// Decompresses whatever [`Format::sniff`] recognises; anything else is returned as is.
pub fn decompress_any(data: Vec<u8>, limit: usize) -> Result<Vec<u8>> {
    match Format::sniff(&data) {
        Some(format) => format.decompress(&data, limit),
        None => Ok(data),
    }
}

fn read_capped(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    reader.take(limit as u64 + 1).read_to_end(&mut out)?;
    ensure!(out.len() <= limit, "output over {limit} bytes");
    Ok(out)
}

#[cfg(test)]
mod tests;
