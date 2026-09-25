//! Recover what the tiles do not show: hidden solid volumes (exact shape, prior material) and
//! game-rule properties.

mod flood;
mod material;
mod rules;

use std::collections::{HashMap, HashSet};

use bmr_invert::face::Cell;
use bmr_invert::{Inverted, Library};
use bmr_world::{BlockRegistry, BlockState};

pub use flood::Bounds;
use material::{Known, materials};

#[derive(Debug, Default)]
pub struct Stats {
    pub observed: usize,
    pub hidden_solid: usize,
    pub leaves_adjusted: usize,
}

/// Observed blocks + filled hidden solids, with game-rule properties recomputed.
pub fn complete(
    inv: &Inverted,
    lib: &Library,
    registry: &BlockRegistry,
    bounds: &Bounds,
) -> (HashMap<Cell, BlockState>, Stats) {
    let observed: HashMap<Cell, &BlockState> = inv.blocks.iter().map(|(&c, &e)| (c, &lib.entries[e].state)).collect();
    let full_cubes: HashSet<Cell> =
        inv.blocks.iter().filter(|(_, e)| lib.entries[**e].full_cube).map(|(&c, _)| c).collect();
    let hidden = flood::hidden_solids(&observed, &inv.occluders, bounds);
    let known = Known { observed: &observed, full_cubes: &full_cubes, hidden: &hidden, min_y: bounds.min_y, registry };

    let mut blocks: HashMap<Cell, BlockState> = observed.iter().map(|(&c, &s)| (c, s.clone())).collect();
    blocks.extend(materials(&known));
    let stats = Stats {
        observed: observed.len(),
        hidden_solid: hidden.len(),
        leaves_adjusted: rules::leaves_distance(&mut blocks),
    };
    (blocks, stats)
}
