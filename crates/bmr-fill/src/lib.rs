//! Recover what the tiles do not show: unseen volumes classified per column gap from face evidence
//! (solid / liquid / air), filled from a same-seed regeneration when available, else from priors;
//! plus game-rule properties.

mod columns;
mod liquid;
mod material;
mod regen;
mod rules;

use std::time::Instant;

use rustc_hash::FxHashMap;

use bmr_invert::face::{Cell, Liquid};
use bmr_invert::timings::Timings;
use bmr_invert::{Inverted, Library};
use bmr_world::{BlockRegistry, BlockState, StateId, StateTable};
use rayon::prelude::*;

pub use columns::{Bounds, Column};
use columns::{Fill, Gap, evidence_by_column, gaps};
use material::{SolidGap, default_block, solid_segments};
pub use regen::RegenWorld;

/// Inclusive y run of one interned state within a column.
#[derive(Clone, Copy)]
pub struct Segment {
    pub column: Column,
    pub ylo: i32,
    pub yhi: i32,
    pub state: StateId,
}

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
    pub blocks: FxHashMap<Cell, BlockState>,
    /// Unseen solid/liquid runs (states interned in the caller's table).
    pub segments: Vec<Segment>,
    pub stats: Stats,
}

pub fn complete(
    inv: &Inverted,
    lib: &Library,
    registry: &BlockRegistry,
    bounds: &Bounds,
    regen: Option<&RegenWorld>,
    table: &mut StateTable,
) -> Filled {
    let named = |name: &str| {
        let props = registry.get(name).map(|b| b.default.clone()).unwrap_or_default();
        BlockState::new(name.to_owned(), props)
    };
    let mut blocks: FxHashMap<Cell, BlockState> =
        inv.blocks.iter().map(|(&c, &e)| (c, lib.entries[e].state.clone())).collect();
    let liquid_states: FxHashMap<Liquid, BlockState> =
        [Liquid::Water, Liquid::Lava].into_iter().map(|l| (l, named(l.block()))).collect();
    blocks.extend(inv.liquids.iter().map(|(&c, l)| (c, liquid_states[l].clone())));

    let mut t = Timings::default();
    let (all_gaps, evidence) = t.time("gaps", || {
        let mut observed_ys: FxHashMap<Column, Vec<i32>> = FxHashMap::default();
        for &(x, y, z) in blocks.keys() {
            observed_ys.entry((x, z)).or_default().push(y);
        }
        let evidence = evidence_by_column(&inv.evidence);
        (gaps(&observed_ys, &evidence, bounds), evidence)
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
            let water = table.intern(&liquid_states[&Liquid::Water]);
            let lava = table.intern(&liquid_states[&Liquid::Lava]);
            let priors: FxHashMap<&str, StateId> = ["minecraft:bedrock", "minecraft:deepslate", "minecraft:stone"]
                .into_iter()
                .map(|n| (n, table.intern(&named(n))))
                .collect();
            let full: Vec<bool> =
                table.iter().map(|(_, s)| lib.find(s).is_some_and(|e| lib.entries[e].full_cube)).collect();
            let air: Vec<bool> = table.iter().map(|(id, _)| table.is_air(id)).collect();
            let min_y = bounds.min_y;
            let prior = move |y: i32| priors[default_block(y, min_y)];
            let liquid = move |l: Liquid| if l == Liquid::Water { water } else { lava };
            let cx = regen::Context {
                regen,
                evidence: &evidence,
                full: &full,
                air: &air,
                prior: &prior,
                liquid: &liquid,
                cave_y: bounds.cave_y,
            };
            all_gaps
                .par_iter()
                .fold(Vec::new, |mut acc, g| {
                    regen::fill_gap_into(g, &cx, &mut acc);
                    acc
                })
                .reduce(Vec::new, |mut a, mut b| {
                    a.append(&mut b);
                    a
                })
        }
        None => prior_segments(inv, lib, registry, bounds, &blocks, &all_gaps, &liquid_states, table),
    };
    t.record("segments", seg_start.elapsed());
    stats.leaves_adjusted = t.time("leaves", || rules::leaves_distance(&mut blocks));
    if let Some(regen) = regen {
        let same_render = |a: &BlockState, b: &BlockState| match (lib.find(a), lib.find(b)) {
            (Some(x), Some(y)) => lib.entries[x].sig == lib.entries[y].sig && lib.entries[x].tint == lib.entries[y].tint,
            _ => false,
        };
        stats.adopted_from_regen =
            t.time("adopt", || regen::adopt_invisible(&mut blocks, regen, table, &same_render));
    }
    stats.timings = t;
    Filled { blocks, segments, stats }
}

/// No regeneration: estimated deep-water floors and material priors.
#[allow(clippy::too_many_arguments)]
fn prior_segments(
    inv: &Inverted,
    lib: &Library,
    registry: &BlockRegistry,
    bounds: &Bounds,
    blocks: &FxHashMap<Cell, BlockState>,
    all_gaps: &[Gap],
    liquid_states: &FxHashMap<Liquid, BlockState>,
    table: &mut StateTable,
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
                let state = table.intern(&liquid_states[&l]);
                segments.push(Segment { column: g.column, ylo: top_of_solid + 1, yhi: g.yhi, state });
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
    for r in solid_segments(&solid, bounds.min_y, registry, &full_cube_at) {
        segments.push(Segment { column: r.column, ylo: r.ylo, yhi: r.yhi, state: table.intern(&r.state) });
    }
    segments
}
