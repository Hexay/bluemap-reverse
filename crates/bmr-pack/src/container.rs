//! Byte-level pack container (layout in format.rs). Packs are downloaded, so every read is bounded.

use std::io::{Read, Write};

use anyhow::{Context, Result, bail, ensure};
use lzma_rust2::{XzOptions, XzReader, XzWriter};

use crate::format::{Body, FORMAT, Header, MAGIC};

/// Uncompressed header or body; real bodies are a few MB.
pub(crate) const MAX_RAW: usize = 256 << 20;
/// Decoder memory: covers the 64 MiB dictionary `compress` writes at most.
const MAX_DECODER_KIB: u32 = 80 << 10;

pub(crate) fn compress(raw: &[u8]) -> Result<Vec<u8>> {
    let mut opts = XzOptions::with_preset(9);
    // the reader allocates the whole declared dictionary (64 MB at preset 9)
    opts.lzma_options.dict_size = (raw.len() as u32).next_power_of_two().clamp(1 << 12, opts.lzma_options.dict_size);
    let mut xz = XzWriter::new(Vec::new(), opts)?;
    xz.write_all(raw)?;
    Ok(xz.finish()?)
}

pub(crate) fn decompress(compressed: &[u8], limit: usize) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    XzReader::new_mem_limit(compressed, false, MAX_DECODER_KIB).take(limit as u64 + 1).read_to_end(&mut raw)?;
    ensure!(raw.len() <= limit, "decompresses to over {limit} bytes");
    Ok(raw)
}

pub(crate) fn read_body(compressed: &[u8]) -> Result<Body> {
    postcard::from_bytes(&decompress(compressed, MAX_RAW)?).context("corrupt pack body")
}

/// (header, compressed body) after validating magic and format.
pub(crate) fn split(bytes: &[u8]) -> Result<(Header, &[u8])> {
    ensure!(bytes.len() > 12 && bytes.starts_with(MAGIC), "not a bmr pack");
    let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let (header, body) = bytes[12..].split_at_checked(len).context("truncated header")?;
    let header: Header = decompress(header, MAX_RAW)
        .ok()
        .and_then(|raw| postcard::from_bytes(&raw).ok())
        .context("unreadable header (older pack format?)")?;
    if header.format != FORMAT {
        bail!("pack format {} not supported by this bmr (expects {FORMAT})", header.format);
    }
    Ok((header, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompress_refuses_output_over_limit() {
        let xz = compress(&[0; 10_000]).unwrap();
        assert_eq!(decompress(&xz, 10_000).unwrap().len(), 10_000);
        let e = decompress(&xz, 9_999).unwrap_err();
        assert!(e.to_string().contains("over 9999 bytes"), "{e}");
    }

    /// Block header (after the 12-byte stream header) rewritten to declare a 4 GiB LZMA2 dictionary.
    fn with_huge_dictionary(mut xz: Vec<u8>) -> Vec<u8> {
        let size = (xz[12] as usize + 1) * 4;
        let filter = xz[12..12 + size].windows(2).position(|w| w == [0x21, 0x01]).unwrap();
        xz[12 + filter + 2] = 40;
        let mut crc = flate2::Crc::new();
        crc.update(&xz[12..12 + size - 4]);
        xz[12 + size - 4..12 + size].copy_from_slice(&crc.sum().to_le_bytes());
        xz
    }

    #[test]
    fn decompress_refuses_huge_declared_dictionary() {
        let xz = with_huge_dictionary(compress(b"pack").unwrap());
        let e = decompress(&xz, MAX_RAW).unwrap_err();
        assert!(e.to_string().contains("mem_limit"), "{e}");
    }
}
