//! Recover what the tiles do not show: unseen volumes classified per column gap from face evidence
//! (solid / liquid / air), filled from a same-seed regeneration when available, else from priors;
//! plus game-rule properties.

mod columns;
mod liquid;
mod material;
mod regen;
mod rules;

use std::collections::HashMap;
use std::time::Instant;

use bmr_invert::face::{Cell, Liquid};
use bmr_invert::timings::Timings;
use bmr_invert::{Inverted, Library};
use bmr_world::{BlockRegistry, BlockState};

pub use columns::{Bounds, Column};
use columns::{Fill, Gap, gaps};
pub use material::Segment;
use material::{SolidGap, default_block, solid_segments};
pub use regen::RegenWorld;

#[derive(Debug, Default)]
pub struct Stats {
    pub observed: usize,
    pub solid_cells: usize,
    pub liquid_cells: usize,
    pub leaves_adjusted: usize,
    /// Observed blocks whose invisible properties were taken from the regeneration.
    pub adopted_from_regen: usize,
    pub timings: Timings,
}

pub struct Filled {
    /// Observed blocks and observed liquid cells, with game-rule properties recomputed.
    pub blocks: HashMap<Cell, BlockState>,
    /// Unseen solid/liquid runs.
    pub segments: Vec<Segment>,
    pub stats: Stats,
}

pub fn complete(
    inv: &Inverted,
    lib: &Library,
    registry: &BlockRegistry,
    bounds: &Bounds,
    regen: Option<&RegenWorld>,
) -> Filled {
    let named = |name: &str| {
        let props = registry.get(name).map(|b| b.default.clone()).unwrap_or_default();
        BlockState::new(name.to_owned(), props)
    };
    let mut blocks: HashMap<Cell, BlockState> =
        inv.blocks.iter().map(|(&c, &e)| (c, lib.entries[e].state.clone())).collect();
    let liquid_states: HashMap<Liquid, BlockState> =
        [Liquid::Water, Liquid::Lava].into_iter().map(|l| (l, named(l.block()))).collect();
    blocks.extend(inv.liquids.iter().map(|(&c, l)| (c, liquid_states[l].clone())));

    let mut t = Timings::default();
    let all_gaps = t.time("gaps", || {
        let mut observed_ys: HashMap<Column, Vec<i32>> = HashMap::new();
        for &(x, y, z) in blocks.keys() {
            observed_ys.entry((x, z)).or_default().push(y);
        }
        gaps(&observed_ys, &inv.evidence, bounds)
    });
    let mut stats = Stats { observed: blocks.len(), ..Stats::default() };
    for g in &all_gaps {
        let n = (g.yhi - g.ylo + 1) as usize;
        match g.fill {
            Fill::Solid => stats.solid_cells += n,
            Fill::Liquid(_) => stats.liquid_cells += n,
            Fill::Air => {}
        }
    }

    let seg_start = Instant::now();
    let segments = match regen {
        Some(regen) => {
            let is_full = |s: &BlockState| lib.find(s).is_some_and(|e| lib.entries[e].full_cube);
            let prior = |(_, y, _): Cell| named(default_block(y, bounds.min_y));
            let liquid = |l: Liquid| liquid_states[&l].clone();
            let cx = regen::Context {
                regen,
                evidence: &inv.evidence,
                is_full: &is_full,
                prior: &prior,
                liquid: &liquid,
                cave_y: bounds.cave_y,
            };
            all_gaps.iter().flat_map(|g| regen::fill_gap(g, &cx)).collect()
        }
        None => prior_segments(inv, lib, registry, bounds, &blocks, &all_gaps, &liquid_states),
    };
    t.0.push(("segments".into(), seg_start.elapsed()));
    stats.leaves_adjusted = t.time("leaves", || rules::leaves_distance(&mut blocks));
    if let Some(regen) = regen {
        let same_render = |a: &BlockState, b: &BlockState| match (lib.find(a), lib.find(b)) {
            (Some(x), Some(y)) => lib.entries[x].sig == lib.entries[y].sig && lib.entries[x].tint == lib.entries[y].tint,
            _ => false,
        };
        stats.adopted_from_regen = t.time("adopt", || regen::adopt_invisible(&mut blocks, regen, &same_render));
    }
    stats.timings = t;
    Filled { blocks, segments, stats }
}

/// No regeneration: estimated deep-water floors and material priors.
fn prior_segments(
    inv: &Inverted,
    lib: &Library,
    registry: &BlockRegistry,
    bounds: &Bounds,
    blocks: &HashMap<Cell, BlockState>,
    all_gaps: &[Gap],
    liquid_states: &HashMap<Liquid, BlockState>,
) -> Vec<Segment> {
    let liquid_gaps: Vec<&Gap> = all_gaps.iter().filter(|g| matches!(g.fill, Fill::Liquid(_))).collect();
    let floors = liquid::estimate_floors(&liquid_gaps);
    let mut segments = Vec::new();
    let mut solid = Vec::new();
    for g in all_gaps {
        match g.fill {
            Fill::Liquid(l) => {
                let floor = floors.get(&g.column).copied().filter(|_| !g.floored);
                let top_of_solid = floor.unwrap_or(g.ylo - 1);
                segments.push(Segment { column: g.column, ylo: top_of_solid + 1, yhi: g.yhi, state: liquid_states[&l].clone() });
                if let Some(f) = floor {
                    solid.push(SolidGap { column: g.column, ylo: g.ylo, yhi: f, above: None });
                }
            }
            Fill::Solid => {
                let above = blocks.get(&(g.column.0, g.yhi + 1, g.column.1));
                solid.push(SolidGap { column: g.column, ylo: g.ylo, yhi: g.yhi, above });
            }
            Fill::Air => {}
        }
    }
    let full_cube_at =
        |c: Cell| inv.blocks.get(&c).filter(|&&e| lib.entries[e].full_cube).map(|&e| lib.entries[e].state.clone());
    segments.extend(solid_segments(&solid, bounds.min_y, registry, &full_cube_at));
    segments
}
