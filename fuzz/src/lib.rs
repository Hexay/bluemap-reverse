//! Shared by the fuzz targets: consumers' accessors run on every decoded value, so a parser that accepts
//! input its readers cannot handle is a finding too.

use bmr_world::{BlockInfo, BlockRegistry, Chunk};

pub fn registry() -> BlockRegistry {
    let slab = BlockInfo {
        properties: vec![("type".into(), vec!["top".into(), "bottom".into()])],
        default: vec![("type".into(), "bottom".into())],
    };
    BlockRegistry::from_blocks([("minecraft:stone_slab".to_owned(), slab)])
}

pub fn decode_chunk(nbt: &[u8], registry: &BlockRegistry) {
    for reg in [None, Some(registry)] {
        if let Ok(chunk) = bmr_world::decode_chunk(nbt, reg) {
            walk(&chunk);
        }
    }
}

fn walk(chunk: &Chunk) {
    for s in &chunk.sections {
        for i in 0..4096 {
            s.block(i % 16, i / 256, i / 16 % 16).to_string();
        }
        for i in 0..64 {
            s.biome(i % 4, i / 16, i / 4 % 4);
        }
    }
    if let Some((lo, hi)) = chunk.y_range() {
        for y in [lo, hi] {
            chunk.block(0, y, 15);
        }
    }
}
