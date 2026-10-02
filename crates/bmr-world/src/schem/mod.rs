//! Sponge Schematic v3 export (`.schem`, read by WorldEdit/FAWE):
//! <https://github.com/SpongePowered/Schematic-Specification/blob/master/versions/schematic-3.md>
//! Unnamed root → `Schematic` compound; `Blocks`/`Biomes` data are LEB128 varints ordered
//! `x + z*W + y*W*L`; biomes are per block; the file is gzipped.

mod format;
mod read;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use fastnbt::IntArray;
use flate2::Compression;
use flate2::write::GzEncoder;
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::block_entities::block_entity_type;
use crate::chunk::Chunk;
use crate::world::{ChunkPos, World};
use format::{Biomes, BlockEntity, Blocks, Metadata, Root, Schematic, block_id, intern, sorted, unix_millis, varints};
pub use read::read_schem;

/// Inclusive block box.
#[derive(Debug, Clone, Copy)]
pub struct Area {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

impl Area {
    fn size(&self) -> [usize; 3] {
        std::array::from_fn(|a| (self.max[a] - self.min[a] + 1) as usize)
    }
}

#[derive(Debug)]
pub struct SchemStats {
    /// The exported box (after trimming).
    pub area: Area,
    pub palette: usize,
    pub non_air: u64,
    pub block_entities: usize,
}

/// Export `area` of `world`. With `trim`, the box shrinks to the non-air blocks inside it.
pub fn export_schem(world: &World, area: Area, trim: bool, name: &str, path: &Path) -> Result<SchemStats> {
    let chunks = load_chunks(world, &area)?;
    let data_version = chunks.values().map(|c| c.data_version).max().context("no chunks in the area")?;
    let area = if trim { non_air_bounds(&chunks, &area).context("no blocks in the area")? } else { area };
    let [w, h, l] = area.size();
    ensure!(w <= 65535 && h <= 65535 && l <= 65535, "schematic too large ({w}x{h}x{l}); use a smaller --area");

    let mut block_palette: FxHashMap<String, u32> = FxHashMap::default();
    block_palette.insert("minecraft:air".into(), 0);
    let mut biome_palette: FxHashMap<String, u32> = FxHashMap::default();
    let mut blocks = vec![0u32; w * h * l];
    let mut biomes = vec![0u32; w * h * l];
    let mut block_entities = Vec::new();
    let mut non_air = 0u64;

    for chunk in chunks.values() {
        for s in &chunk.sections {
            // section palette → schematic palette once, not per cell
            let ids: Vec<u32> = s.palette.iter().map(|b| block_id(&mut block_palette, b)).collect();
            let biome_ids: Vec<u32> = s.biome_palette.iter().map(|b| intern(&mut biome_palette, b)).collect();
            let be: Vec<Option<&str>> = s.palette.iter().map(|b| block_entity_type(&b.name)).collect();
            for ly in 0..16 {
                let y = s.y * 16 + ly;
                if y < area.min[1] || y > area.max[1] {
                    continue;
                }
                for lz in 0..16 {
                    let z = chunk.z * 16 + lz;
                    if z < area.min[2] || z > area.max[2] {
                        continue;
                    }
                    for lx in 0..16 {
                        let x = chunk.x * 16 + lx;
                        if x < area.min[0] || x > area.max[0] {
                            continue;
                        }
                        let (rx, ry, rz) =
                            ((x - area.min[0]) as usize, (y - area.min[1]) as usize, (z - area.min[2]) as usize);
                        let i = rx + rz * w + ry * w * l;
                        let p = s.index(((ly * 16 + lz) * 16 + lx) as usize) as usize;
                        blocks[i] = ids[p];
                        let bi = s.biomes.get(((ly as usize / 4) * 4 + lz as usize / 4) * 4 + lx as usize / 4);
                        biomes[i] = bi.map_or(0, |&b| biome_ids.get(b as usize).copied().unwrap_or(0));
                        if ids[p] != 0 {
                            non_air += 1;
                        }
                        if let Some(t) = be[p] {
                            block_entities.push(BlockEntity {
                                pos: IntArray::new(vec![rx as i32, ry as i32, rz as i32]),
                                id: format!("minecraft:{t}"),
                                data: BTreeMap::new(),
                            });
                        }
                    }
                }
            }
        }
    }

    let stats = SchemStats { area, palette: block_palette.len(), non_air, block_entities: block_entities.len() };
    let root = Root {
        schematic: Schematic {
            version: 3,
            data_version,
            metadata: Metadata { name: name.to_owned(), date: unix_millis() },
            width: w as u16 as i16,
            height: h as u16 as i16,
            length: l as u16 as i16,
            offset: IntArray::new(vec![0, 0, 0]),
            blocks: Blocks { palette: sorted(block_palette), data: varints(&blocks), block_entities },
            biomes: Biomes { palette: sorted(biome_palette), data: varints(&biomes) },
        },
    };
    let nbt = fastnbt::to_bytes(&root)?;
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&nbt)?;
    std::fs::write(path, gz.finish()?).with_context(|| path.display().to_string())?;
    Ok(stats)
}

fn load_chunks(world: &World, area: &Area) -> Result<FxHashMap<ChunkPos, Chunk>> {
    let (cx0, cz0, cx1, cz1) = (
        area.min[0].div_euclid(16),
        area.min[2].div_euclid(16),
        area.max[0].div_euclid(16),
        area.max[2].div_euclid(16),
    );
    let inside = |(x, z): ChunkPos| (cx0..=cx1).contains(&x) && (cz0..=cz1).contains(&z);
    let regions: Vec<_> = (cx0.div_euclid(32)..=cx1.div_euclid(32))
        .flat_map(|rx| (cz0.div_euclid(32)..=cz1.div_euclid(32)).map(move |rz| (rx, rz)))
        .collect();
    let per_region = regions.par_iter().map(|&r| world.read_region_where(r, &inside)).collect::<Result<Vec<_>>>()?;
    Ok(per_region.into_iter().flatten().collect())
}

/// Smallest box inside `area` holding every non-air block.
fn non_air_bounds(chunks: &FxHashMap<ChunkPos, Chunk>, area: &Area) -> Option<Area> {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for c in chunks.values() {
        for s in &c.sections {
            if s.blocks.is_empty() && s.palette[0].is_air() {
                continue;
            }
            for i in 0..4096 {
                if s.palette[s.index(i) as usize].is_air() {
                    continue;
                }
                let p = [c.x * 16 + (i % 16) as i32, s.y * 16 + (i / 256) as i32, c.z * 16 + (i / 16 % 16) as i32];
                if (0..3).all(|a| (area.min[a]..=area.max[a]).contains(&p[a])) {
                    for a in 0..3 {
                        lo[a] = lo[a].min(p[a]);
                        hi[a] = hi[a].max(p[a]);
                    }
                }
            }
        }
    }
    (lo[0] <= hi[0]).then_some(Area { min: lo, max: hi })
}
