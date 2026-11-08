//! Random block access to a few chunks of a world (the ones candidate structure placements cover).

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use bmr_world::coords::{chunk_of, local, region_of_chunk};
use bmr_world::{Chunk, World};
use rayon::prelude::*;

/// Block names (without `minecraft:`) by world position; `None` = unknown or not loaded.
pub trait BlockSource {
    fn name_at(&self, p: [i32; 3]) -> Option<&str>;
}

pub struct Blocks {
    chunks: HashMap<[i32; 2], Chunk>,
}

impl Blocks {
    pub fn load(world: &World, chunks: &HashSet<[i32; 2]>) -> Result<Self> {
        let regions: HashSet<(i32, i32)> = chunks.iter().map(|c| region_of_chunk(c[0], c[1])).collect();
        let regions: Vec<(i32, i32)> = regions.into_iter().collect();
        let keep = |(x, z): (i32, i32)| chunks.contains(&[x, z]);
        let loaded: Vec<_> = regions.par_iter().map(|&r| world.read_region_where(r, &keep)).collect::<Result<_>>()?;
        let chunks = loaded.into_iter().flatten().map(|((x, z), c)| ([x, z], c)).collect();
        Ok(Self { chunks })
    }
}

impl BlockSource for Blocks {
    fn name_at(&self, [x, y, z]: [i32; 3]) -> Option<&str> {
        let (cx, cz) = chunk_of(x, z);
        let chunk = self.chunks.get(&[cx, cz])?;
        let name = &chunk.block(local(x), y, local(z))?.name;
        Some(name.strip_prefix("minecraft:").unwrap_or(name))
    }
}

impl BlockSource for HashMap<[i32; 3], String> {
    fn name_at(&self, p: [i32; 3]) -> Option<&str> {
        self.get(&p).map(String::as_str)
    }
}
