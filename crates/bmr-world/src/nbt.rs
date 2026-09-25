//! Minimal serde view of chunk NBT (1.18+ layout, still current on 26.3). Unknown keys are ignored,
//! so the struct survives format drift outside the fields we read.

use std::collections::BTreeMap;

use anyhow::{Result, bail, ensure};
use serde::Deserialize;

use crate::chunk::{BlockState, Chunk, Section};
use crate::registry::BlockRegistry;

#[derive(Deserialize)]
struct ChunkNbt {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "xPos")]
    x: i32,
    #[serde(rename = "zPos")]
    z: i32,
    #[serde(rename = "Status", alias = "status", default)]
    status: String,
    #[serde(default)]
    sections: Vec<SectionNbt>,
}

#[derive(Deserialize)]
struct SectionNbt {
    #[serde(rename = "Y")]
    y: i8,
    block_states: Option<Paletted<StateNbt>>,
    biomes: Option<Paletted<String>>,
}

#[derive(Deserialize)]
struct Paletted<T> {
    palette: Vec<T>,
    data: Option<fastnbt::LongArray>,
}

/// Palette entry encodings seen in the wild:
/// - legacy (≤ 26.2?): `{Name, Properties?}` with every property
/// - 26.3: `{id, properties}` with every property, or a bare name meaning the **default state**; mixed
///   lists wrap bare names as `{"": name}`, all-bare lists are plain string lists.
#[derive(Deserialize)]
#[serde(untagged)]
enum StateNbt {
    Bare(String),
    Compound {
        #[serde(rename = "")]
        bare: Option<String>,
        #[serde(rename = "id", alias = "Name")]
        id: Option<String>,
        #[serde(rename = "properties", alias = "Properties", default)]
        properties: BTreeMap<String, String>,
    },
}

fn resolve(entry: StateNbt, registry: Option<&BlockRegistry>) -> Result<BlockState> {
    let (name, props) = match entry {
        StateNbt::Bare(name) | StateNbt::Compound { bare: Some(name), .. } => {
            let Some(reg) = registry else {
                bail!("palette uses the 26.3 default-state shorthand ({name}); a block registry is required");
            };
            let default = reg.get(&name).map(|b| b.default.clone()).unwrap_or_default();
            (name, default)
        }
        StateNbt::Compound { id: Some(name), properties, .. } => (name, properties.into_iter().collect()),
        StateNbt::Compound { .. } => bail!("palette entry without a name"),
    };
    Ok(BlockState::new(name, props))
}

pub fn decode_chunk(nbt: &[u8], registry: Option<&BlockRegistry>) -> Result<Chunk> {
    let c: ChunkNbt = fastnbt::from_bytes(nbt)?;
    let mut sections = Vec::with_capacity(c.sections.len());
    for s in c.sections {
        let Some(bs) = s.block_states else { continue };
        let palette: Vec<BlockState> =
            bs.palette.into_iter().map(|p| resolve(p, registry)).collect::<Result<_>>()?;
        let blocks = unpack(bs.data.as_deref(), palette.len(), 4096, 4)?;
        let (biome_palette, biomes) = match s.biomes {
            Some(b) => {
                let idx = unpack(b.data.as_deref(), b.palette.len(), 64, 1)?;
                (b.palette, idx)
            }
            None => (Vec::new(), vec![0; 64]),
        };
        sections.push(Section { y: s.y as i32, palette, blocks, biome_palette, biomes });
    }
    sections.sort_by_key(|s| s.y);
    Ok(Chunk { x: c.x, z: c.z, data_version: c.data_version, status: c.status, sections })
}

/// Paletted container: `bits = max(min_bits, ceil(log2(len)))`, entries never span longs (1.16+).
/// Single-entry palettes have no data array.
pub fn unpack(data: Option<&[i64]>, palette_len: usize, count: usize, min_bits: u32) -> Result<Vec<u16>> {
    let Some(data) = data else { return Ok(vec![0; count]) };
    if palette_len <= 1 {
        return Ok(vec![0; count]);
    }
    let bits = min_bits.max(usize::BITS - (palette_len - 1).leading_zeros());
    let per_long = (64 / bits) as usize;
    ensure!(data.len() * per_long >= count, "packed array too short ({} longs, {bits} bits)", data.len());
    let mask = (1u64 << bits) - 1;
    let out: Vec<u16> =
        (0..count).map(|i| ((data[i / per_long] as u64 >> ((i % per_long) as u32 * bits)) & mask) as u16).collect();
    ensure!(out.iter().all(|&i| (i as usize) < palette_len), "palette index out of range");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_bit_entries_do_not_span_longs() {
        // 17-entry palette → 5 bits, 12 per long; entry 12 starts the second long
        let mut longs = vec![0i64; 342];
        longs[0] = (3 << 5) | (16 << 55);
        longs[1] = 7;
        let v = unpack(Some(&longs), 17, 4096, 4).unwrap();
        assert_eq!((v[0], v[1], v[11], v[12]), (0, 3, 16, 7));
    }

    #[test]
    fn biome_bits_can_be_one() {
        let v = unpack(Some(&[0b10]), 2, 64, 1).unwrap();
        assert_eq!((v[0], v[1], v[2]), (0, 1, 0));
    }
}
