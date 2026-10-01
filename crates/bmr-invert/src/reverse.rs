//! Mirror → matched cells + evidence. Cells are gathered across all tiles first, so geometry that
//! overhangs into a neighbouring cell (sign boards, fire, …) can be credited to its block.
//! Liquid faces are split off before matching: their shape depends on neighbours (see evidence.rs).

use std::collections::BTreeMap;

use rustc_hash::{FxHashMap, FxHashSet};

use anyhow::Result;
use bmr_fetch::LocalMap;
use bmr_fetch::grid::Tile;
use rayon::prelude::*;

use crate::evidence::{Evidence, Observed, collect};
use crate::face::{Cell, CellFaces, FaceKey, Liquid, Tex, faces_by_cell, signature, texture_ids, world_faces};
use crate::library::Library;
use crate::matcher::{Candidates, How, candidates, candidates_cullable, resolve};
use crate::overhang::{self, Matched};
use crate::timings::Timings;
use crate::tints::{self, TintSum};

pub struct Inverted {
    /// Matched non-liquid blocks → library entry (waterlogged already resolved).
    pub blocks: FxHashMap<Cell, usize>,
    /// Cells showing nothing but liquid.
    pub liquids: FxHashMap<Cell, Liquid>,
    pub evidence: Evidence,
    /// Biome tints seen per 4×4 column cell (x >> 2, z >> 2).
    pub tints: Tints,
    pub stats: Stats,
}

#[derive(Debug, Default)]
pub struct Stats {
    pub cells: usize,
    pub by_how: BTreeMap<String, usize>,
    pub liquid_cells: usize,
    pub waterlogged: usize,
    /// Cells whose faces were all explained as a neighbour's overhang.
    pub overhang_cells: usize,
    /// Unmatched cells that matched after dropping faces a neighbour may have overhung into them.
    pub foreign_stripped: usize,
    pub unmatched: usize,
    /// Unmatched cells by their texture set, most frequent first.
    pub unmatched_textures: Vec<(String, usize)>,
    pub timings: Timings,
}

/// Interned texture per material index of a map (`textures.json`); parse once, reuse per window.
pub fn map_textures(map: &LocalMap) -> Result<Vec<Tex>> {
    Ok(texture_ids(&bmr_prbm::parse_texture_names(&map.textures_json()?)?))
}

/// Invert the given hires tiles (a window plus halo, or the whole map). Cells near the edge of the
/// tile set lack neighbours, so only use results at least one cell inside it.
pub fn reverse(map: &LocalMap, lib: &Library, tiles: &[Tile], textures: &[Tex]) -> Result<Inverted> {
    let mut t = Timings::default();
    let (cells, tints) = t.time("gather", || gather(map, tiles, textures))?;
    let (mut solid_faces, liquid_faces) = t.time("split_liquid", || split_liquid(cells));
    let mut matched = t.time("match", || match_all(lib, &solid_faces));
    let mut stats = Stats::default();
    t.time("overhang", || {
        stats.overhang_cells = overhang::credit(lib, &mut solid_faces, &mut matched);
        stats.foreign_stripped = overhang::strip_foreign(lib, &mut solid_faces, &mut matched);
        stats.overhang_cells += overhang::credit(lib, &mut solid_faces, &mut matched);
    });
    t.time("match_cullable", || match_cullable(lib, &solid_faces, &mut matched));

    let liquids: FxHashMap<Cell, Liquid> = liquid_faces
        .iter()
        .filter(|(c, _)| !solid_faces.contains_key(c))
        .filter_map(|(&c, keys)| keys[0].liquid().map(|l| (c, l)))
        .collect();
    stats.liquid_cells = liquids.len();

    let mut unmatched: FxHashMap<String, usize> = FxHashMap::default();
    let mut blocks = FxHashMap::default();
    for (cell, m) in matched {
        stats.cells += 1;
        match m {
            Some((entry, how)) => {
                *stats.by_how.entry(how_name(how).into()).or_default() += 1;
                blocks.insert(cell, entry);
            }
            None => {
                stats.unmatched += 1;
                *unmatched.entry(texture_set(&solid_faces[&cell].keys)).or_default() += 1;
            }
        }
    }
    let mut u: Vec<_> = unmatched.into_iter().collect();
    u.sort_by_key(|e| std::cmp::Reverse(e.1));
    stats.unmatched_textures = u;

    let obs = Observed { blocks: &blocks, solid_faces: &solid_faces, liquid_faces: &liquid_faces, liquids: &liquids };
    let evidence = t.time("evidence", || collect(lib, &obs));
    for (cell, entry) in blocks.iter_mut() {
        let liquid = liquid_faces.get(cell).and_then(|k| k[0].liquid()).or_else(|| evidence.liquid.get(cell).copied());
        // both ways: ties between wet and dry (identical solid faces) resolve to the default, e.g. wet coral
        if let Some(w) = lib.liquid_variant(*entry, liquid) {
            *entry = w;
            stats.waterlogged += liquid.is_some() as usize;
        }
    }
    stats.timings = t;
    Ok(Inverted { blocks, liquids, evidence, tints, stats })
}

fn split_liquid(cells: FxHashMap<Cell, CellFaces>) -> (FxHashMap<Cell, CellFaces>, FxHashMap<Cell, Vec<FaceKey>>) {
    let mut solid = FxHashMap::with_capacity_and_hasher(cells.len(), Default::default());
    let mut liquid = FxHashMap::default();
    for (cell, mut obs) in cells {
        // most cells have no liquid faces: move them through without reallocating
        if !obs.keys.iter().any(|k| k.liquid().is_some()) {
            solid.insert(cell, obs);
            continue;
        }
        let (wet, dry): (Vec<FaceKey>, Vec<FaceKey>) = std::mem::take(&mut obs.keys).into_iter().partition(|k| k.liquid().is_some());
        if !wet.is_empty() {
            liquid.insert(cell, wet);
        }
        if !dry.is_empty() {
            obs.keys = dry;
            solid.insert(cell, obs);
        }
    }
    (solid, liquid)
}

/// Every cell BlueMap drew at least one face for (the "visible" set for scoring).
pub fn rendered_cells(map: &LocalMap) -> Result<FxHashSet<Cell>> {
    Ok(gather(map, &map.tiles(0), &map_textures(map)?)?.0.into_keys().collect())
}

type Tints = FxHashMap<(i32, i32), TintSum>;

/// Faces per cell, and biome tints per 4×4 column cell.
fn gather(map: &LocalMap, tiles: &[Tile], names: &[Tex]) -> Result<(FxHashMap<Cell, CellFaces>, Tints)> {
    let per_tile = tiles
        .par_iter()
        .map(|&t| -> Result<_> {
            let tile = bmr_prbm::parse(&map.tile_bytes(0, t)?)?;
            let faces = world_faces(&tile, map.hires_origin(t), names);
            let mut tints = Tints::default();
            tints::collect(&faces, &mut tints);
            Ok((faces_by_cell(&faces), tints))
        })
        .collect::<Result<Vec<_>>>()?;
    let total: usize = per_tile.iter().map(|t| t.0.len()).sum();
    let mut cells: FxHashMap<Cell, CellFaces> = FxHashMap::with_capacity_and_hasher(total, Default::default());
    let mut tints = Tints::default();
    for (tile_cells, tile_tints) in per_tile {
        for (cell, obs) in tile_cells {
            cells.entry(cell).or_default().merge(obs);
        }
        for (col, t) in tile_tints {
            tints.entry(col).or_default().merge(&t);
        }
    }
    Ok((cells, tints))
}

/// Distinct signatures are few compared to cells (terrain repeats), so each is matched exactly once.
fn match_all(lib: &Library, cells: &FxHashMap<Cell, CellFaces>) -> FxHashMap<Cell, Matched> {
    let sigs: Vec<(Cell, Vec<FaceKey>, &CellFaces)> =
        cells.par_iter().map(|(&cell, obs)| (cell, signature(obs.keys.clone()), obs)).collect();
    let mut unique: FxHashMap<&[FaceKey], usize> = FxHashMap::default();
    for (_, sig, _) in &sigs {
        let n = unique.len();
        unique.entry(sig.as_slice()).or_insert(n);
    }
    let mut distinct: Vec<(&[FaceKey], usize)> = unique.iter().map(|(&s, &i)| (s, i)).collect();
    distinct.sort_unstable_by_key(|&(_, i)| i);
    let results: Vec<Option<Candidates>> = distinct.par_iter().map(|&(sig, _)| candidates(lib, sig)).collect();
    sigs.par_iter()
        .map(|(cell, sig, obs)| {
            let c = &results[unique[sig.as_slice()]];
            (*cell, c.as_ref().map(|c| (resolve(lib, &c.ids, obs), c.how)))
        })
        .collect()
}

fn match_cullable(lib: &Library, cells: &FxHashMap<Cell, CellFaces>, matched: &mut FxHashMap<Cell, Matched>) {
    for (cell, m) in matched.iter_mut().filter(|(_, m)| m.is_none()) {
        let obs = &cells[cell];
        *m = candidates_cullable(lib, &signature(obs.keys.clone())).map(|c| (resolve(lib, &c.ids, obs), c.how));
    }
}

fn how_name(h: How) -> &'static str {
    match h {
        How::Exact => "exact",
        How::Offset => "offset",
        How::Partial => "partial",
    }
}

fn texture_set(keys: &[FaceKey]) -> String {
    let names: Vec<_> = keys.iter().map(|k| k.texture.name()).collect();
    let mut t: Vec<&str> = names.iter().map(|n| n.trim_start_matches("minecraft:block/")).collect();
    t.sort();
    t.dedup();
    t.join("+")
}
