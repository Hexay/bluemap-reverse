//! Chunk → NBT in the exact shape a 26.3 server writes (palette rules: see nbt.rs `StateNbt`).
//! Heightmaps and light are omitted: the server recomputes them (`isLightOn` absent → relight).

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use fastnbt::LongArray;
use serde::Serialize;

use crate::block_entities::block_entity_type;
use crate::chunk::{BlockState, Chunk};
use crate::registry::BlockRegistry;

#[derive(Serialize)]
struct ChunkOut {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "xPos")]
    x: i32,
    #[serde(rename = "zPos")]
    z: i32,
    #[serde(rename = "yPos")]
    y: i32,
    #[serde(rename = "Status")]
    status: String,
    sections: Vec<SectionOut>,
    block_entities: Vec<BlockEntityOut>,
}

/// Minimal block entity: the server fills every other field with defaults.
#[derive(Serialize)]
struct BlockEntityOut {
    id: String,
    x: i32,
    y: i32,
    z: i32,
    #[serde(rename = "keepPacked")]
    keep_packed: bool,
}

#[derive(Serialize)]
struct SectionOut {
    #[serde(rename = "Y")]
    y: i8,
    block_states: PalettedOut<EntryOut>,
    biomes: PalettedOut<String>,
}

#[derive(Serialize)]
struct PalettedOut<T> {
    palette: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<LongArray>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum EntryOut {
    Bare(String),
    Wrapped {
        #[serde(rename = "")]
        name: String,
    },
    Full {
        id: String,
        properties: BTreeMap<String, String>,
    },
}

pub fn encode_chunk(chunk: &Chunk, registry: &BlockRegistry) -> Result<Vec<u8>> {
    let mut sections = Vec::with_capacity(chunk.sections.len());
    for s in &chunk.sections {
        let palette = encode_palette(&s.palette, registry)?;
        sections.push(SectionOut {
            y: s.y as i8,
            block_states: PalettedOut { palette, data: pack(&s.blocks, s.palette.len(), 4) },
            biomes: PalettedOut { palette: s.biome_palette.clone(), data: pack(&s.biomes, s.biome_palette.len(), 1) },
        });
    }
    let min_section = chunk.sections.first().map_or(-4, |s| s.y);
    let out = ChunkOut {
        data_version: chunk.data_version,
        x: chunk.x,
        z: chunk.z,
        y: min_section,
        status: chunk.status.clone(),
        sections,
        block_entities: block_entities(chunk),
    };
    Ok(fastnbt::to_bytes(&out)?)
}

fn block_entities(chunk: &Chunk) -> Vec<BlockEntityOut> {
    let mut out = Vec::new();
    for s in &chunk.sections {
        let types: Vec<Option<&str>> = s.palette.iter().map(|b| block_entity_type(&b.name)).collect();
        if types.iter().all(Option::is_none) {
            continue;
        }
        for i in 0..4096 {
            if let Some(t) = types[s.index(i) as usize] {
                out.push(BlockEntityOut {
                    id: format!("minecraft:{t}"),
                    x: chunk.x * 16 + (i % 16) as i32,
                    y: s.y * 16 + (i / 256) as i32,
                    z: chunk.z * 16 + (i / 16 % 16) as i32,
                    keep_packed: false,
                });
            }
        }
    }
    out
}

fn encode_palette(palette: &[BlockState], registry: &BlockRegistry) -> Result<Vec<EntryOut>> {
    let mut defaults = Vec::with_capacity(palette.len());
    for s in palette {
        let Some(info) = registry.get(&s.name) else { bail!("unknown block {} (not in blocks.json)", s.name) };
        defaults.push(info.default == s.properties);
    }
    if defaults.iter().all(|&d| d) {
        return Ok(palette.iter().map(|s| EntryOut::Bare(s.name.clone())).collect());
    }
    Ok(palette
        .iter()
        .zip(defaults)
        .map(|(s, default)| match default {
            true => EntryOut::Wrapped { name: s.name.clone() },
            false => EntryOut::Full { id: s.name.clone(), properties: s.properties.iter().cloned().collect() },
        })
        .collect())
}

/// Inverse of `nbt::unpack`.
fn pack(indices: &[u16], palette_len: usize, min_bits: u32) -> Option<LongArray> {
    if palette_len <= 1 {
        return None;
    }
    let bits = min_bits.max(usize::BITS - (palette_len - 1).leading_zeros());
    let per_long = (64 / bits) as usize;
    let mut longs = vec![0i64; indices.len().div_ceil(per_long)];
    for (i, &v) in indices.iter().enumerate() {
        longs[i / per_long] |= ((v as u64) << ((i % per_long) as u32 * bits)) as i64;
    }
    Some(LongArray::new(longs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbt::unpack;

    #[test]
    fn pack_roundtrips_unpack() {
        for len in [2usize, 5, 17, 300] {
            let idx: Vec<u16> = (0..4096).map(|i| (i * 7 % len) as u16).collect();
            let packed = pack(&idx, len, 4).unwrap();
            assert_eq!(unpack(Some(&packed), len, 4096, 4).unwrap(), idx, "palette {len}");
        }
    }
}
