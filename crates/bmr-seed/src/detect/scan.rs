//! Marker blocks of a world: positions of every block whose name is in a marker set, per set.

use anyhow::Result;
use bmr_world::World;
use rayon::prelude::*;

pub struct Markers {
    /// per marker set, the positions of its blocks
    pub positions: Vec<Vec<[i32; 3]>>,
    /// every chunk present in the world
    pub chunks: Vec<[i32; 2]>,
}

pub fn marker_positions(world: &World, marker_sets: &[&[&str]]) -> Result<Markers> {
    let regions = world.regions()?;
    let per_region: Vec<Markers> = regions
        .par_iter()
        .map(|&r| -> Result<_> {
            let mut out = vec![Vec::new(); marker_sets.len()];
            let mut chunks = Vec::new();
            for (_, chunk) in world.read_region(r)? {
                chunks.push([chunk.x, chunk.z]);
                for s in &chunk.sections {
                    // palette index → which marker sets it belongs to
                    let hits: Vec<Vec<usize>> = s
                        .palette
                        .iter()
                        .map(|p| {
                            let name = p.name.strip_prefix("minecraft:").unwrap_or(&p.name);
                            (0..marker_sets.len()).filter(|&m| marker_sets[m].contains(&name)).collect()
                        })
                        .collect();
                    if hits.iter().all(Vec::is_empty) {
                        continue;
                    }
                    for i in 0..4096 {
                        for &m in &hits[s.index(i) as usize] {
                            let (x, y, z) = (i % 16, i / 256, (i / 16) % 16);
                            out[m].push([chunk.x * 16 + x as i32, s.y * 16 + y as i32, chunk.z * 16 + z as i32]);
                        }
                    }
                }
            }
            Ok(Markers { positions: out, chunks })
        })
        .collect::<Result<_>>()?;
    let mut merged = Markers { positions: vec![Vec::new(); marker_sets.len()], chunks: Vec::new() };
    for region in per_region {
        for (m, pts) in region.positions.into_iter().enumerate() {
            merged.positions[m].extend(pts);
        }
        merged.chunks.extend(region.chunks);
    }
    Ok(merged)
}
