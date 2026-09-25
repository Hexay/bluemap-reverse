//! Mirror → matched cells + evidence. Cells are gathered across all tiles first, so geometry that
//! overhangs into a neighbouring cell (sign boards, fire, …) can be credited to its block.
//! Liquid faces are split off before matching: their shape depends on neighbours (see evidence.rs).

use std::collections::BTreeMap;

use rustc_hash::{FxHashMap, FxHashSet};

use anyhow::Result;
use bmr_fetch::LocalMap;
use rayon::prelude::*;

use crate::evidence::{Evidence, Observed, collect};
use crate::face::{Cell, CellFaces, FaceKey, Liquid, faces_by_cell, signature, step, texture_ids, world_faces};
use crate::library::Library;
use crate::matcher::{Candidates, How, candidates, resolve};
use crate::timings::Timings;

pub struct Inverted {
    /// Matched non-liquid blocks → library entry (waterlogged already resolved).
    pub blocks: FxHashMap<Cell, usize>,
    /// Cells showing nothing but liquid.
    pub liquids: FxHashMap<Cell, Liquid>,
    pub evidence: Evidence,
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
    pub unmatched: usize,
    /// Unmatched cells by their texture set, most frequent first.
    pub unmatched_textures: Vec<(String, usize)>,
    pub timings: Timings,
}

type Matched = Option<(usize, How)>;

pub fn reverse(map: &LocalMap, lib: &Library) -> Result<Inverted> {
    let mut t = Timings::default();
    let cells = t.time("gather", || gather(map))?;
    let (mut solid_faces, liquid_faces) = t.time("split_liquid", || split_liquid(cells));
    let mut matched = t.time("match", || match_all(lib, &solid_faces));
    let mut stats = Stats::default();
    stats.overhang_cells = t.time("overhang", || credit_overhang(lib, &mut solid_faces, &mut matched));

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
    u.sort_by(|a, b| b.1.cmp(&a.1));
    stats.unmatched_textures = u;

    let obs = Observed { blocks: &blocks, solid_faces: &solid_faces, liquid_faces: &liquid_faces, liquids: &liquids };
    let evidence = t.time("evidence", || collect(lib, &obs));
    for (cell, entry) in blocks.iter_mut() {
        let wet = liquid_faces.contains_key(cell) || evidence.liquid.contains_key(cell);
        if let Some(w) = wet.then(|| lib.waterlogged_variant(*entry)).flatten() {
            *entry = w;
            stats.waterlogged += 1;
        }
    }
    stats.timings = t;
    Ok(Inverted { blocks, liquids, evidence, stats })
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

/// Remove faces a matched block overhangs into its neighbours, re-match those neighbours.
/// Returns the number of cells that turned out to hold nothing but overhang.
fn credit_overhang(lib: &Library, cells: &mut FxHashMap<Cell, CellFaces>, matched: &mut FxHashMap<Cell, Matched>) -> usize {
    let mut touched = FxHashSet::default();
    for (&cell, m) in matched.iter() {
        let Some((entry, _)) = m else { continue };
        for (off, key) in &lib.entries[*entry].overhang {
            let n = step(cell, *off);
            if let Some(obs) = cells.get_mut(&n) {
                if let Some(i) = obs.keys.iter().position(|k| k == key) {
                    obs.keys.swap_remove(i);
                    touched.insert(n);
                }
            }
        }
    }
    let mut emptied = 0;
    for n in touched {
        if cells[&n].keys.is_empty() {
            matched.remove(&n);
            cells.remove(&n);
            emptied += 1;
        } else {
            let obs = &cells[&n];
            let sig = signature(obs.keys.clone());
            matched.insert(n, candidates(lib, &sig).map(|c| (resolve(lib, &c.ids, obs.tint()), c.how)));
        }
    }
    emptied
}

/// Every cell BlueMap drew at least one face for (the "visible" set for scoring).
pub fn rendered_cells(map: &LocalMap) -> Result<FxHashSet<Cell>> {
    Ok(gather(map)?.into_keys().collect())
}

fn gather(map: &LocalMap) -> Result<FxHashMap<Cell, CellFaces>> {
    let names = texture_ids(&bmr_prbm::parse_texture_names(&map.textures_json()?)?);
    let per_tile = map
        .tiles(0)
        .par_iter()
        .map(|&t| -> Result<_> {
            let tile = bmr_prbm::parse(&map.tile_bytes(0, t)?)?;
            Ok(faces_by_cell(&world_faces(&tile, map.hires_origin(t), &names)))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut cells: FxHashMap<Cell, CellFaces> = FxHashMap::default();
    for tile_cells in per_tile {
        for (cell, obs) in tile_cells {
            cells.entry(cell).or_default().merge(obs);
        }
    }
    Ok(cells)
}

/// Distinct signatures are few compared to cells (terrain repeats), so each is matched exactly once.
fn match_all(lib: &Library, cells: &FxHashMap<Cell, CellFaces>) -> FxHashMap<Cell, Matched> {
    let sigs: Vec<(Cell, Vec<FaceKey>, Option<[u8; 3]>)> =
        cells.par_iter().map(|(&cell, obs)| (cell, signature(obs.keys.clone()), obs.tint())).collect();
    let mut unique: FxHashMap<&[FaceKey], usize> = FxHashMap::default();
    for (_, sig, _) in &sigs {
        let n = unique.len();
        unique.entry(sig.as_slice()).or_insert(n);
    }
    let mut distinct: Vec<(&[FaceKey], usize)> = unique.iter().map(|(&s, &i)| (s, i)).collect();
    distinct.sort_unstable_by_key(|&(_, i)| i);
    let results: Vec<Option<Candidates>> = distinct.par_iter().map(|&(sig, _)| candidates(lib, sig)).collect();
    sigs.par_iter()
        .map(|(cell, sig, tint)| {
            let c = &results[unique[sig.as_slice()]];
            (*cell, c.as_ref().map(|c| (resolve(lib, &c.ids, *tint), c.how)))
        })
        .collect()
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
