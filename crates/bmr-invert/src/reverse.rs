//! Mirror → matched cells + evidence. Cells are gathered across all tiles first, so geometry that
//! overhangs into a neighbouring cell (sign boards, fire, …) can be credited to its block.
//! Liquid faces are split off before matching: their shape depends on neighbours (see evidence.rs).

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Result;
use bmr_fetch::LocalMap;
use rayon::prelude::*;

use crate::evidence::{Evidence, Observed, collect};
use crate::face::{Cell, CellFaces, FaceKey, Liquid, faces_by_cell, signature, step, texture_names, world_faces};
use crate::library::Library;
use crate::matcher::{Candidates, How, candidates, resolve};

pub struct Inverted {
    /// Matched non-liquid blocks → library entry (waterlogged already resolved).
    pub blocks: HashMap<Cell, usize>,
    /// Cells showing nothing but liquid.
    pub liquids: HashMap<Cell, Liquid>,
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
}

type Matched = Option<(usize, How)>;

pub fn reverse(map: &LocalMap, lib: &Library) -> Result<Inverted> {
    let (mut solid_faces, liquid_faces) = split_liquid(gather(map)?);
    let mut matched = match_all(lib, &solid_faces);
    let mut stats = Stats::default();
    stats.overhang_cells = credit_overhang(lib, &mut solid_faces, &mut matched);

    let liquids: HashMap<Cell, Liquid> = liquid_faces
        .iter()
        .filter(|(c, _)| !solid_faces.contains_key(c))
        .filter_map(|(&c, keys)| keys[0].liquid().map(|l| (c, l)))
        .collect();
    stats.liquid_cells = liquids.len();

    let mut unmatched: HashMap<String, usize> = HashMap::new();
    let mut blocks = HashMap::new();
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
    let evidence = collect(lib, &obs);
    for (cell, entry) in blocks.iter_mut() {
        let wet = liquid_faces.contains_key(cell) || evidence.liquid.contains_key(cell);
        if let Some(w) = wet.then(|| lib.waterlogged_variant(*entry)).flatten() {
            *entry = w;
            stats.waterlogged += 1;
        }
    }
    Ok(Inverted { blocks, liquids, evidence, stats })
}

fn split_liquid(cells: HashMap<Cell, CellFaces>) -> (HashMap<Cell, CellFaces>, HashMap<Cell, Vec<FaceKey>>) {
    let mut solid = HashMap::new();
    let mut liquid = HashMap::new();
    for (cell, obs) in cells {
        let (wet, dry): (Vec<FaceKey>, Vec<FaceKey>) = obs.keys.iter().cloned().partition(|k| k.liquid().is_some());
        if !wet.is_empty() {
            liquid.insert(cell, wet);
        }
        if !dry.is_empty() {
            let mut o = obs;
            o.keys = dry;
            solid.insert(cell, o);
        }
    }
    (solid, liquid)
}

/// Remove faces a matched block overhangs into its neighbours, re-match those neighbours.
/// Returns the number of cells that turned out to hold nothing but overhang.
fn credit_overhang(lib: &Library, cells: &mut HashMap<Cell, CellFaces>, matched: &mut HashMap<Cell, Matched>) -> usize {
    let mut touched = HashSet::new();
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
pub fn rendered_cells(map: &LocalMap) -> Result<HashSet<Cell>> {
    Ok(gather(map)?.into_keys().collect())
}

fn gather(map: &LocalMap) -> Result<HashMap<Cell, CellFaces>> {
    let textures = bmr_prbm::parse_textures(&map.textures_json()?)?;
    let names = texture_names(&textures);
    let per_tile = map
        .tiles(0)
        .par_iter()
        .map(|&t| -> Result<_> {
            let tile = bmr_prbm::parse(&map.tile_bytes(0, t)?)?;
            Ok(faces_by_cell(&world_faces(&tile, map.hires_origin(t), &names)))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut cells: HashMap<Cell, CellFaces> = HashMap::new();
    for tile_cells in per_tile {
        for (cell, obs) in tile_cells {
            cells.entry(cell).or_default().merge(obs);
        }
    }
    Ok(cells)
}

fn match_all(lib: &Library, cells: &HashMap<Cell, CellFaces>) -> HashMap<Cell, Matched> {
    type Cache = HashMap<Vec<FaceKey>, Option<Candidates>>;
    cells
        .par_iter()
        .map_init(Cache::new, |cache, (&cell, obs)| {
            let sig = signature(obs.keys.clone());
            let c = cache.entry(sig.clone()).or_insert_with(|| candidates(lib, &sig));
            (cell, c.as_ref().map(|c| (resolve(lib, &c.ids, obs.tint()), c.how)))
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
    let mut t: Vec<&str> = keys.iter().map(|k| k.texture.as_ref().trim_start_matches("minecraft:block/")).collect();
    t.sort();
    t.dedup();
    t.join("+")
}
