//! Mirror → chunks. Cells are gathered across all tiles first, so geometry that overhangs into a
//! neighbouring cell (sign boards, fire, …) can be credited to its block and removed from the neighbour.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Result;
use bmr_fetch::LocalMap;
use rayon::prelude::*;

use crate::face::{Cell, CellFaces, FaceKey, faces_by_cell, signature, texture_names, world_faces};
use crate::library::Library;
use crate::matcher::{Candidates, How, candidates, resolve};

/// Result of inverting the visible geometry.
pub struct Inverted {
    /// Cell → library entry.
    pub blocks: HashMap<Cell, usize>,
    /// Unmatched-by-geometry cells that must hold a full opaque block: a neighbour's cullable face
    /// towards them is missing.
    pub occluders: HashSet<Cell>,
    pub stats: Stats,
}

#[derive(Debug, Default)]
pub struct Stats {
    pub cells: usize,
    pub by_how: BTreeMap<String, usize>,
    /// Cells whose faces were all explained as a neighbour's overhang.
    pub overhang_cells: usize,
    pub unmatched: usize,
    /// Unmatched cells by their texture set, most frequent first.
    pub unmatched_textures: Vec<(String, usize)>,
}

type Matched = Option<(usize, How)>;

pub fn reverse(map: &LocalMap, lib: &Library) -> Result<Inverted> {
    let mut cells = gather(map)?;
    let mut matched = match_all(lib, &cells);

    // credit overhanging faces to their block, then re-match the neighbours that lost faces
    let mut touched = HashSet::new();
    for (&cell, m) in &matched {
        let Some((entry, _)) = m else { continue };
        for (off, key) in &lib.entries[*entry].overhang {
            let n = (cell.0 + off.0, cell.1 + off.1, cell.2 + off.2);
            if let Some(obs) = cells.get_mut(&n) {
                if let Some(i) = obs.keys.iter().position(|k| k == key) {
                    obs.keys.swap_remove(i);
                    touched.insert(n);
                }
            }
        }
    }
    let mut stats = Stats::default();
    for n in touched {
        let obs = &cells[&n];
        if obs.keys.is_empty() {
            matched.remove(&n);
            cells.remove(&n);
            stats.overhang_cells += 1;
        } else {
            let sig = signature(obs.keys.clone());
            matched.insert(n, candidates(lib, &sig).map(|c| (resolve(lib, &c.ids, obs.tint()), c.how)));
        }
    }

    let mut unmatched: HashMap<String, usize> = HashMap::new();
    let mut blocks = HashMap::new();
    for (cell, m) in matched {
        stats.cells += 1;
        let Some((entry, how)) = m else {
            stats.unmatched += 1;
            *unmatched.entry(texture_set(&cells[&cell].keys)).or_default() += 1;
            continue;
        };
        *stats.by_how.entry(how_name(how).into()).or_default() += 1;
        blocks.insert(cell, entry);
    }
    let mut u: Vec<_> = unmatched.into_iter().collect();
    u.sort_by(|a, b| b.1.cmp(&a.1));
    stats.unmatched_textures = u;
    let occluders = occluders(lib, &blocks, &cells);
    Ok(Inverted { blocks, occluders, stats })
}

/// Neighbours across each boundary face the matched state should show but the map does not.
fn occluders(lib: &Library, blocks: &HashMap<Cell, usize>, cells: &HashMap<Cell, CellFaces>) -> HashSet<Cell> {
    let mut out = HashSet::new();
    for (&(x, y, z), &entry) in blocks {
        let observed = cells.get(&(x, y, z)).map_or(&[][..], |c| &c.keys[..]);
        let mut remaining: Vec<&FaceKey> = observed.iter().collect();
        for key in &lib.entries[entry].sig {
            if let Some(i) = remaining.iter().position(|k| *k == key) {
                remaining.swap_remove(i);
                continue;
            }
            if let Some((dx, dy, dz)) = key.boundary_dir() {
                let n = (x + dx, y + dy, z + dz);
                if !blocks.contains_key(&n) {
                    out.insert(n);
                }
            }
        }
    }
    out
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
