//! Wave-parallel flood fill over a tile layer: probe seeds, then 4-neighbours of every present tile.

use std::collections::BTreeSet;

use anyhow::Result;
use rayon::prelude::*;

use crate::grid::{Tile, neighbours};
use crate::store::Probed;

/// `fetch` returns whether the tile exists. `on_wave` runs after each wave (persist progress).
/// Resumable: neighbours of already-present tiles are re-queued, known tiles are never re-probed.
pub fn discover(
    probed: &mut Probed,
    seeds: impl IntoIterator<Item = Tile>,
    fetch: impl Fn(Tile) -> Result<bool> + Sync,
    mut on_wave: impl FnMut(&Probed) -> Result<()>,
) -> Result<()> {
    let resumed: Vec<Tile> = probed.present.iter().flat_map(|&t| neighbours(t)).collect();
    let mut frontier: BTreeSet<Tile> = seeds.into_iter().chain(resumed).filter(|t| !probed.known(t)).collect();

    while !frontier.is_empty() {
        let results: Vec<(Tile, Result<bool>)> = frontier.par_iter().map(|&t| (t, fetch(t))).collect();
        let mut next = BTreeSet::new();
        let mut first_err = None;
        for (t, r) in results {
            match r {
                Ok(true) => {
                    probed.present.insert(t);
                    next.extend(neighbours(t));
                }
                Ok(false) => {
                    probed.empty.insert(t);
                }
                Err(e) => {
                    first_err.get_or_insert(e);
                }
            }
        }
        on_wave(probed)?;
        if let Some(e) = first_err {
            return Err(e);
        }
        frontier = next.into_iter().filter(|t| !probed.known(t)).collect();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_connected_region_only() {
        let exists = |(x, z): Tile| (0..3).contains(&x) && (0..2).contains(&z) || (x, z) == (10, 10);
        let mut p = Probed::default();
        discover(&mut p, [(1, 1)], |t| Ok(exists(t)), |_| Ok(())).unwrap();
        assert_eq!(p.present.len(), 6);
        assert!(!p.present.contains(&(10, 10)));
        // perimeter of a 3×2 block
        assert_eq!(p.empty.len(), 10);
    }
}
