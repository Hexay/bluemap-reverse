//! Map-wide histogram of drawn faces by block height: the evidence `bmr_fill::infer` reads the map's render
//! settings from. One pass over every hires tile, kept separate from the windowed inversion because the
//! settings must be known before the first window is filled.

use std::collections::BTreeMap;

use anyhow::Result;
use bmr_fetch::LocalMap;
use bmr_prbm::{Face, Tile};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

/// BlueMap's default `cave-detection-ocean-floor` (-5) under the heightmap (top block + 1), plus a block of slack.
const OCEAN_FLOOR_EXEMPT: i32 = 5;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DrawnHeights {
    /// Triangles per owner block y.
    pub faces: BTreeMap<i32, u64>,
    /// Triangles BlueMap's cave culling would have dropped below `remove-caves-below-y`, per owner block y:
    /// sunlight and blocklight 0 (culled whichever light the map tests) and more than 5 blocks under the
    /// column's ocean-floor heightmap (culling never reaches above that).
    pub cave: BTreeMap<i32, u64>,
}

impl DrawnHeights {
    /// Every hires tile of the map.
    pub fn scan(map: &LocalMap) -> Result<Self> {
        let names = bmr_prbm::parse_texture_names(&map.textures_json()?)?;
        let liquid: Vec<bool> = names.iter().map(|n| n.contains("water") || n.contains("lava")).collect();
        map.tiles(0)
            .par_iter()
            .map(|&t| Ok(Self::of_tile(&bmr_prbm::parse(&map.tile_bytes(0, t)?)?, &liquid)))
            .try_reduce(Self::default, |mut a, b| {
                a.merge(b);
                Ok(a)
            })
    }

    /// `liquid`: per material, whether it is a liquid texture.
    pub fn of_tile(tile: &Tile, liquid: &[bool]) -> Self {
        let is_liquid = |f: &Face| liquid.get(f.material as usize).copied().unwrap_or(false);
        // highest full non-liquid top face per column: a lower bound on the ocean-floor heightmap (every such
        // block blocks motion)
        let mut floor: FxHashMap<(i32, i32), i32> = FxHashMap::default();
        for f in tile.faces().filter(|f| full_top(f) && !is_liquid(f)) {
            let (x, y, z) = owner(&f);
            let e = floor.entry((x, z)).or_insert(y);
            *e = (*e).max(y);
        }
        let mut h = Self::default();
        for f in tile.faces() {
            let (x, y, z) = owner(&f);
            *h.faces.entry(y).or_default() += 1;
            let buried = floor.get(&(x, z)).is_some_and(|&top| y < top - OCEAN_FLOOR_EXEMPT);
            if f.sunlight <= 0 && f.blocklight <= 0 && buried {
                *h.cave.entry(y).or_default() += 1;
            }
        }
        h
    }

    fn merge(&mut self, o: Self) {
        for (y, n) in o.faces {
            *self.faces.entry(y).or_default() += n;
        }
        for (y, n) in o.cave {
            *self.cave.entry(y).or_default() += n;
        }
    }

    /// Lowest and highest drawn block y.
    pub fn range(&self) -> Option<(i32, i32)> {
        Some((*self.faces.keys().next()?, *self.faces.keys().next_back()?))
    }
}

/// The block a face belongs to: the one it faces out of (as `WorldFace::owner`), in tile-local x/z.
fn owner(f: &Face) -> (i32, i32, i32) {
    let p: [i32; 3] = std::array::from_fn(|a| {
        let c = f.pos.iter().map(|v| v[a]).sum::<f32>() / 3.0;
        (c - f32::from(f.normal[a]) / 127.0 * 1e-3).floor() as i32
    });
    (p[0], p[1], p[2])
}

/// Half of a whole cell's top: facing straight up at a block boundary, corners on the cell's corners.
fn full_top(f: &Face) -> bool {
    let y = f.pos[0][1];
    f.normal == [0, 127, 0]
        && y.fract() == 0.0
        && f.pos.iter().all(|p| p[1] == y && p[0].fract() == 0.0 && p[2].fract() == 0.0)
}

#[cfg(test)]
#[path = "drawn_tests.rs"]
mod tests;
