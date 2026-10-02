//! Geometry drawn outside its block's cell (sign boards, fire, iron-bar caps, …) lands in a neighbour's
//! observed faces and has to be taken back out before that neighbour can match.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::face::{Cell, CellFaces, FaceKey, signature, step};
use crate::library::Library;
use crate::matcher::{How, candidates, resolve};

pub type Matched = Option<(usize, How)>;

/// Remove faces a matched block overhangs into its neighbours, re-match those neighbours.
/// Returns the number of cells that turned out to hold nothing but overhang.
pub fn credit(lib: &Library, cells: &mut FxHashMap<Cell, CellFaces>, matched: &mut FxHashMap<Cell, Matched>) -> usize {
    let mut touched = FxHashSet::default();
    for (&cell, m) in matched.iter() {
        let Some((entry, _)) = m else { continue };
        for (off, key) in &lib.entries[*entry].overhang {
            let n = step(cell, *off);
            if let Some(obs) = cells.get_mut(&n)
                && let Some(i) = obs.keys.iter().position(|k| k == key)
            {
                obs.keys.swap_remove(i);
                touched.insert(n);
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
            rematch(lib, cells, matched, n);
        }
    }
    emptied
}

/// Neighbours that overhang into each other (stacked iron bars trade cap faces) both stay unmatched, so
/// `credit` never runs for either. For each still-unmatched cell, drop the faces some library state could
/// have overhung into it from an observed neighbour, and match the rest. Returns the cells now matched.
pub fn strip_foreign(
    lib: &Library,
    cells: &mut FxHashMap<Cell, CellFaces>,
    matched: &mut FxHashMap<Cell, Matched>,
) -> usize {
    let stripped: Vec<(Cell, Vec<FaceKey>)> = matched
        .iter()
        .filter(|(_, m)| m.is_none())
        .filter_map(|(&n, _)| {
            let keys = &cells[&n].keys;
            let foreign = |k: &FaceKey| {
                lib.overhang_from(k).iter().any(|&(dx, dy, dz)| cells.contains_key(&step(n, (-dx, -dy, -dz))))
            };
            let own: Vec<FaceKey> = keys.iter().copied().filter(|k| !foreign(k)).collect();
            (!own.is_empty() && own.len() < keys.len()).then_some((n, own))
        })
        .collect();
    let mut fixed = 0;
    for (n, own) in stripped {
        if candidates(lib, &signature(own.clone())).is_some() {
            cells.get_mut(&n).expect("observed").keys = own;
            rematch(lib, cells, matched, n);
            fixed += 1;
        }
    }
    fixed
}

fn rematch(lib: &Library, cells: &FxHashMap<Cell, CellFaces>, matched: &mut FxHashMap<Cell, Matched>, n: Cell) {
    let obs = &cells[&n];
    let sig = signature(obs.keys.clone());
    matched.insert(n, candidates(lib, &sig).map(|c| (resolve(lib, &c.ids, obs), c.how)));
}
