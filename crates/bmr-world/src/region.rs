//! Anvil `.mca` container: 1024 chunk slots, 4 KiB sectors, per-chunk compression byte.
//! https://minecraft.wiki/w/Region_file_format

use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use flate2::read::{GzDecoder, ZlibDecoder};

const SECTOR: usize = 4096;

/// Decompressed chunk NBT for every present slot, keyed by local (x, z) in 0..32.
pub fn read_region(path: &Path) -> Result<Vec<((u8, u8), Vec<u8>)>> {
    let data = std::fs::read(path).with_context(|| path.display().to_string())?;
    if data.is_empty() {
        return Ok(Vec::new());
    }
    ensure!(data.len() >= 2 * SECTOR, "{}: truncated header", path.display());
    let mut out = Vec::new();
    for slot in 0..1024 {
        let loc = u32::from_be_bytes(data[slot * 4..slot * 4 + 4].try_into().unwrap());
        if loc == 0 {
            continue;
        }
        let offset = (loc >> 8) as usize * SECTOR;
        let nbt = read_chunk(&data, offset).with_context(|| format!("{} slot {slot}", path.display()))?;
        out.push((((slot % 32) as u8, (slot / 32) as u8), nbt));
    }
    Ok(out)
}

fn read_chunk(data: &[u8], offset: usize) -> Result<Vec<u8>> {
    ensure!(offset + 5 <= data.len(), "chunk offset past end of file");
    let len = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    ensure!(len >= 1 && offset + 4 + len <= data.len(), "bad chunk length {len}");
    let kind = data[offset + 4];
    let body = &data[offset + 5..offset + 4 + len];
    // bit 7 = payload stored externally in c.<x>.<z>.mcc (oversized chunks)
    ensure!(kind & 0x80 == 0, "external .mcc chunk not supported yet");
    let mut out = Vec::new();
    match kind {
        1 => GzDecoder::new(body).read_to_end(&mut out)?,
        2 => ZlibDecoder::new(body).read_to_end(&mut out)?,
        3 => return Ok(body.to_vec()),
        // TODO: LZ4 (type 4, region-file-compression=lz4 servers)
        k => bail!("unsupported chunk compression {k}"),
    };
    Ok(out)
}
