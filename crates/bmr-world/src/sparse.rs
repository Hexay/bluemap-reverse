//! Sparse block map → full chunks for the writer.

use std::collections::HashMap;

use crate::chunk::{BlockState, Chunk};
use crate::world::ChunkPos;

pub struct ChunkLayout {
    pub data_version: i32,
    /// Inclusive section range written for every touched chunk (overworld: -4..=19).
    pub sections: (i32, i32),
    pub biome: String,
}

pub fn chunks_from_blocks<'a>(
    blocks: impl IntoIterator<Item = ((i32, i32, i32), &'a BlockState)>,
    layout: &ChunkLayout,
) -> Vec<Chunk> {
    let mut chunks: HashMap<ChunkPos, Chunk> = HashMap::new();
    for ((x, y, z), state) in blocks {
        let pos = (x.div_euclid(16), z.div_euclid(16));
        let chunk = chunks
            .entry(pos)
            .or_insert_with(|| Chunk::new(pos.0, pos.1, layout.data_version, layout.sections, &layout.biome));
        chunk.set_block(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize, state);
    }
    let mut out: Vec<Chunk> = chunks.into_values().collect();
    for c in &mut out {
        for s in &mut c.sections {
            s.compact();
        }
    }
    out
}
