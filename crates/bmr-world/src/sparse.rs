//! Sparse blocks + column segments → full chunks for the writer.

use std::collections::HashMap;

use crate::chunk::{BlockState, Chunk};
use crate::world::ChunkPos;

pub struct ChunkLayout {
    pub data_version: i32,
    /// Inclusive section range written for every touched chunk (overworld: -4..=19).
    pub sections: (i32, i32),
    pub biome: String,
}

pub struct ChunkBuilder {
    layout: ChunkLayout,
    chunks: HashMap<ChunkPos, Chunk>,
}

impl ChunkBuilder {
    pub fn new(layout: ChunkLayout) -> Self {
        Self { layout, chunks: HashMap::new() }
    }

    fn chunk(&mut self, x: i32, z: i32) -> &mut Chunk {
        let pos = (x.div_euclid(16), z.div_euclid(16));
        let l = &self.layout;
        self.chunks.entry(pos).or_insert_with(|| Chunk::new(pos.0, pos.1, l.data_version, l.sections, &l.biome))
    }

    pub fn set_block(&mut self, (x, y, z): (i32, i32, i32), state: &BlockState) {
        self.chunk(x, z).set_block(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize, state);
    }

    /// Set `ylo..=yhi` of column (x, z).
    pub fn fill_column(&mut self, (x, z): (i32, i32), ylo: i32, yhi: i32, state: &BlockState) {
        let (lx, lz) = (x.rem_euclid(16) as usize, z.rem_euclid(16) as usize);
        let chunk = self.chunk(x, z);
        for y in ylo..=yhi {
            chunk.set_block(lx, y, lz, state);
        }
    }

    pub fn finish(self) -> Vec<Chunk> {
        let mut out: Vec<Chunk> = self.chunks.into_values().collect();
        for c in &mut out {
            for s in &mut c.sections {
                s.compact();
            }
        }
        out
    }
}
