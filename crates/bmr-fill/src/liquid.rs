//! Liquid gaps without an observed floor. Skylight fades ~1 level per block of water, and BlueMap drops
//! dark faces below `remove-caves-below-y`, so deep ocean floors are simply absent: the gap would run
//! to bedrock. Estimate the floor from the nearest floored liquid columns; it can be no shallower than
//! where darkness starts.

use rustc_hash::FxHashMap;

use crate::columns::{Column, Gap, ring};

/// Water depth at which the floor turns dark enough to be culled.
const DARK_DEPTH: i32 = 15;
/// How far to look for a column whose floor is visible (Chebyshev radius).
const SEARCH_RADIUS: i32 = 32;

/// Liquid gap → y of the estimated top floor block (liquid fills above it, solid from it down).
pub fn estimate_floors(liquid_gaps: &[&Gap]) -> FxHashMap<Column, i32> {
    let floors: FxHashMap<Column, i32> =
        liquid_gaps.iter().filter(|g| g.floored).map(|g| (g.column, g.ylo - 1)).collect();
    liquid_gaps
        .iter()
        .filter(|g| !g.floored)
        .map(|g| {
            let dark_limit = g.yhi - DARK_DEPTH;
            let nearest = (1..=SEARCH_RADIUS).find_map(|r| {
                let found: Vec<i32> =
                    ring(r).filter_map(|(dx, dz)| floors.get(&(g.column.0 + dx, g.column.1 + dz)).copied()).collect();
                (!found.is_empty()).then(|| found.iter().sum::<i32>() / found.len() as i32)
            });
            let floor = nearest.map_or(dark_limit, |f| f.min(dark_limit)).max(g.ylo);
            (g.column, floor)
        })
        .collect()
}
