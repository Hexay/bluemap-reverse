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
    let mut claims: Vec<(Cell, FaceKey, Cell)> = Vec::new();
    for (&cell, m) in matched.iter() {
        let Some((entry, _)) = m else { continue };
        for (off, key) in &lib.entries[*entry].overhang {
            let n = step(cell, *off);
            if cells.get(&n).is_some_and(|obs| obs.keys.contains(key)) {
                claims.push((n, *key, cell));
            }
        }
    }
    // A cell made only of overhang (a floor fire's sides land in the air beside it and match as wall fire) must
    // not claim: it would strip the real block's own faces. Its faces are explained by a matched neighbour's
    // claim, or by a bigger observed neighbour still unmatched (that block's own extra faces may be what blocks
    // it; two lone wall-fire planes side by side must not explain each other away).
    let size = |c: Cell| cells[&c].keys.len();
    let mut claimed: FxHashMap<Cell, Vec<(FaceKey, Cell)>> = FxHashMap::default();
    for (n, key, source) in &claims {
        claimed.entry(*n).or_default().push((*key, *source));
    }
    let pure_by = |counts: &dyn Fn(Cell, Cell) -> bool| -> FxHashSet<Cell> {
        let unmatched_bigger = |s: Cell, src: Cell| matches!(matched.get(&src), Some(None)) && size(src) > size(s);
        let explained = |s: Cell, k: &FaceKey| {
            claimed.get(&s).is_some_and(|c| c.iter().any(|(ck, src)| ck == k && counts(s, *src)))
                || lib.overhang_from(k).iter().any(|&(dx, dy, dz)| unmatched_bigger(s, step(s, (-dx, -dy, -dz))))
        };
        claims.iter().map(|c| c.2).filter(|&s| cells[&s].keys.iter().all(|k| explained(s, k))).collect()
    };
    let candidates = pure_by(&|_, _| true);
    // Cells can explain each other away (a spawner's inner faces land in all its neighbours, and each lone face
    // claims one of the spawner's back): between two such cells, one that shows more of its matched state's
    // signature is the real block; on a tie both stay (isolated wall-fire planes, which each match exactly).
    let coverage = |c: Cell| {
        let sig = matched[&c].map_or(&[][..], |(e, _)| &lib.entries[e].sig[..]);
        cells[&c].keys.iter().filter(|k| sig.contains(k)).count() as f32 / sig.len().max(1) as f32
    };
    let pure = pure_by(&|s, src| !candidates.contains(&src) || coverage(src) >= coverage(s));
    let mut touched = FxHashSet::default();
    for (n, key, source) in claims {
        if pure.contains(&source) {
            continue;
        }
        let obs = cells.get_mut(&n).expect("claimed cells are observed");
        if let Some(i) = obs.keys.iter().position(|k| *k == key) {
            obs.keys.swap_remove(i);
            touched.insert(n);
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
    let stripped: Vec<(Cell, Vec<Vec<FaceKey>>)> = matched
        .iter()
        .filter(|(_, m)| m.is_none())
        .filter_map(|(&n, _)| {
            let keys = &cells[&n].keys;
            let foreign = |k: &FaceKey| {
                lib.overhang_from(k).iter().any(|&(dx, dy, dz)| cells.contains_key(&step(n, (-dx, -dy, -dz))))
            };
            // fewest faces first: a neighbour's plane landing on this block's own plane (a row of fires) is an
            // exact duplicate, and stripping every foreign-looking face would take the block's own sides too
            let mut duplicates_dropped = Vec::with_capacity(keys.len());
            for k in keys {
                if !(foreign(k) && duplicates_dropped.contains(k)) {
                    duplicates_dropped.push(*k);
                }
            }
            let own: Vec<FaceKey> = keys.iter().copied().filter(|k| !foreign(k)).collect();
            let attempts: Vec<Vec<FaceKey>> =
                [duplicates_dropped, own].into_iter().filter(|a| !a.is_empty() && a.len() < keys.len()).collect();
            (!attempts.is_empty()).then_some((n, attempts))
        })
        .collect();
    let mut fixed = 0;
    for (n, attempts) in stripped {
        if let Some(own) = attempts.into_iter().find(|a| candidates(lib, &signature(a.clone())).is_some()) {
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

#[cfg(test)]
#[path = "overhang_tests.rs"]
mod tests;
