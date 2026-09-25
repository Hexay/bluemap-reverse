//! Recover what the tiles do not show: unseen volumes classified per column gap from face evidence
//! (solid / liquid / air), a material prior for solids, and game-rule properties.

mod columns;
mod liquid;
mod material;
mod rules;

use std::collections::HashMap;

use bmr_invert::face::{Cell, Liquid};
use bmr_invert::{Inverted, Library};
use bmr_world::{BlockRegistry, BlockState};

pub use columns::{Bounds, Column};
use columns::{Fill, gaps};
pub use material::Segment;
use material::{SolidGap, solid_segments};

#[derive(Debug, Default)]
pub struct Stats {
    pub observed: usize,
    pub solid_cells: usize,
    pub liquid_cells: usize,
    pub leaves_adjusted: usize,
}

pub struct Filled {
    /// Observed blocks and observed liquid cells, with game-rule properties recomputed.
    pub blocks: HashMap<Cell, BlockState>,
    /// Unseen solid/liquid runs.
    pub segments: Vec<Segment>,
    pub stats: Stats,
}

pub fn complete(inv: &Inverted, lib: &Library, registry: &BlockRegistry, bounds: &Bounds) -> Filled {
    let named = |name: &str| {
        let props = registry.get(name).map(|b| b.default.clone()).unwrap_or_default();
        BlockState::new(name.to_owned(), props)
    };
    let mut blocks: HashMap<Cell, BlockState> =
        inv.blocks.iter().map(|(&c, &e)| (c, lib.entries[e].state.clone())).collect();
    let liquid_states: HashMap<Liquid, BlockState> =
        [Liquid::Water, Liquid::Lava].into_iter().map(|l| (l, named(l.block()))).collect();
    blocks.extend(inv.liquids.iter().map(|(&c, l)| (c, liquid_states[l].clone())));

    let mut observed_ys: HashMap<Column, Vec<i32>> = HashMap::new();
    for &(x, y, z) in blocks.keys() {
        observed_ys.entry((x, z)).or_default().push(y);
    }
    let mut stats = Stats { observed: blocks.len(), ..Stats::default() };
    let mut segments = Vec::new();
    let mut solid = Vec::new();
    let all_gaps = gaps(&observed_ys, &inv.evidence, bounds);
    let liquid_gaps: Vec<&columns::Gap> = all_gaps.iter().filter(|g| matches!(g.fill, Fill::Liquid(_))).collect();
    let floors = liquid::estimate_floors(&liquid_gaps);
    for g in &all_gaps {
        let above = blocks.get(&(g.column.0, g.yhi + 1, g.column.1));
        match g.fill {
            Fill::Liquid(l) => {
                let floor = floors.get(&g.column).copied().filter(|_| !g.floored);
                let top_of_solid = floor.unwrap_or(g.ylo - 1);
                stats.liquid_cells += (g.yhi - top_of_solid) as usize;
                segments.push(Segment { column: g.column, ylo: top_of_solid + 1, yhi: g.yhi, state: liquid_states[&l].clone() });
                if let Some(f) = floor {
                    stats.solid_cells += (f - g.ylo + 1) as usize;
                    solid.push(SolidGap { column: g.column, ylo: g.ylo, yhi: f, above: None });
                }
            }
            Fill::Solid => {
                stats.solid_cells += (g.yhi - g.ylo + 1) as usize;
                solid.push(SolidGap { column: g.column, ylo: g.ylo, yhi: g.yhi, above });
            }
        }
    }
    let full_cube_at = |c: Cell| inv.blocks.get(&c).filter(|&&e| lib.entries[e].full_cube).map(|&e| lib.entries[e].state.clone());
    segments.extend(solid_segments(&solid, bounds.min_y, registry, &full_cube_at));
    stats.leaves_adjusted = rules::leaves_distance(&mut blocks);
    Filled { blocks, segments, stats }
}
