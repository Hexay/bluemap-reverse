//! Sparse blocks + column segments → full chunks for the writer. Built on `StateId` palettes; real
//! `BlockState` palettes are produced once in `finish`.

use rustc_hash::FxHashMap;

use crate::chunk::{Chunk, Section};
use crate::states::{StateId, StateTable};
use crate::world::ChunkPos;

pub struct ChunkLayout {
    pub data_version: i32,
    /// Inclusive section range written for every touched chunk (overworld: -4..=19).
    pub sections: (i32, i32),
    pub biome: String,
}

struct IdSection {
    palette: Vec<StateId>,
    /// Empty = uniform `palette[0]`, as in `Section`.
    blocks: Vec<u16>,
}

impl IdSection {
    fn index_of(&mut self, id: StateId) -> u16 {
        match self.palette.iter().position(|&p| p == id) {
            Some(i) => i as u16,
            None => {
                self.palette.push(id);
                (self.palette.len() - 1) as u16
            }
        }
    }
}

pub struct ChunkBuilder {
    layout: ChunkLayout,
    air: StateId,
    chunks: FxHashMap<ChunkPos, Vec<IdSection>>,
}

impl ChunkBuilder {
    /// `air` must be the id of `minecraft:air` in the table later passed to `finish`.
    pub fn new(layout: ChunkLayout, air: StateId) -> Self {
        Self { layout, air, chunks: FxHashMap::default() }
    }

    fn sections(&mut self, x: i32, z: i32) -> &mut Vec<IdSection> {
        let (lo, hi) = self.layout.sections;
        let air = self.air;
        self.chunks.entry((x.div_euclid(16), z.div_euclid(16))).or_insert_with(|| {
            (lo..=hi).map(|_| IdSection { palette: vec![air], blocks: Vec::new() }).collect()
        })
    }

    /// Make sure the chunk of column (x, z) is written, even if it stays all air (else the game would
    /// generate it with the template's generator and biome).
    pub fn touch(&mut self, (x, z): (i32, i32)) {
        self.sections(x, z);
    }

    pub fn set_block(&mut self, (x, y, z): (i32, i32, i32), id: StateId) {
        self.fill_column((x, z), y, y, id);
    }

    /// Set `ylo..=yhi` of column (x, z); palette lookup once per touched section.
    pub fn fill_column(&mut self, (x, z): (i32, i32), ylo: i32, yhi: i32, id: StateId) {
        let (min_s, max_s) = self.layout.sections;
        let (lx, lz) = (x.rem_euclid(16) as usize, z.rem_euclid(16) as usize);
        let (ylo, yhi) = (ylo.max(min_s * 16), yhi.min(max_s * 16 + 15));
        let sections = self.sections(x, z);
        let mut y = ylo;
        while y <= yhi {
            let sy = y.div_euclid(16);
            let top = yhi.min(sy * 16 + 15);
            let s = &mut sections[(sy - min_s) as usize];
            let idx = s.index_of(id);
            // sections stay uniform (no index array) until something other than palette[0] lands
            if s.blocks.is_empty() && idx != 0 {
                s.blocks = vec![0; 4096];
            }
            if s.blocks.is_empty() {
                y = top + 1;
                continue;
            }
            for yy in y..=top {
                s.blocks[((yy.rem_euclid(16) as usize) * 16 + lz) * 16 + lx] = idx;
            }
            y = top + 1;
        }
    }

    pub fn finish(self, table: &StateTable) -> Vec<Chunk> {
        let (min_s, _) = self.layout.sections;
        let l = &self.layout;
        let mut out: Vec<Chunk> = self
            .chunks
            .into_iter()
            .map(|((cx, cz), sections)| {
                let mut chunk = Chunk::new(cx, cz, l.data_version, l.sections, &l.biome);
                for (i, s) in sections.into_iter().enumerate() {
                    let dst: &mut Section = &mut chunk.sections[i];
                    debug_assert_eq!(dst.y, min_s + i as i32);
                    dst.palette = s.palette.iter().map(|&id| table.get(id).clone()).collect();
                    dst.blocks = s.blocks;
                }
                chunk
            })
            .collect();
        for c in &mut out {
            for s in &mut c.sections {
                s.compact();
            }
        }
        out
    }
}
