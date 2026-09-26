//! A waterlogged block deep in water draws no water faces, and neither do the water cells around it (water
//! against water is culled), so the inversion sees a dry block. But a dry partial block next to water would
//! make that water draw a face towards it: an unobserved water neighbour means the block holds water too.

use rustc_hash::FxHashMap;

use bmr_invert::face::{Cell, DIRS, Liquid};
use bmr_invert::library::Library;
use bmr_world::{BlockState, StateId};

use crate::Segment;
use crate::columns::Column;

/// `water`: the water source state in the segments' table. Returns the number of blocks switched to their
/// waterlogged state.
pub fn from_water_segments(blocks: &mut FxHashMap<Cell, BlockState>, segments: &[Segment], water: StateId, lib: &Library) -> usize {
    let mut runs: FxHashMap<Column, Vec<(i32, i32)>> = FxHashMap::default();
    for s in segments.iter().filter(|s| s.state == water) {
        runs.entry(s.column).or_default().push((s.ylo, s.yhi));
    }
    let water = runs;
    let in_water = |(x, y, z): Cell| water.get(&(x, z)).is_some_and(|runs| runs.iter().any(|&(lo, hi)| (lo..=hi).contains(&y)));
    let updates: Vec<(Cell, BlockState)> = blocks
        .iter()
        .filter(|(_, s)| s.properties.iter().any(|(k, v)| k == "waterlogged" && v == "false"))
        .filter_map(|(&c, s)| {
            let id = lib.find(s)?;
            if lib.entries[id].full_cube {
                return None;
            }
            let touches = DIRS.iter().any(|&(dx, dy, dz)| {
                let n = (c.0 + dx, c.1 + dy, c.2 + dz);
                !blocks.contains_key(&n) && in_water(n)
            });
            let wet = touches.then(|| lib.liquid_variant(id, Some(Liquid::Water))).flatten()?;
            Some((c, lib.entries[wet].state.clone()))
        })
        .collect();
    let n = updates.len();
    blocks.extend(updates);
    n
}
