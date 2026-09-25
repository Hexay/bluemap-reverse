//! Unseen cells from a same-seed regeneration of the untouched terrain, decided per cell against face
//! evidence (gap-level classification is too coarse: one gap can hold a lit cavity above dark rock):
//! - solid evidence: regen if it is a full block there, else the height prior
//! - open evidence: air, unless regen has something non-full (water, plants)
//! - no evidence: solid gap → regen; liquid gap with an observed floor → the liquid (a player pool must
//!   not get regen dirt back), liquid gap running to the world floor → regen (real deep ocean floors,
//!   dark caves below them), liquid only where regen is air above the cave cut-off;
//!   air gap → regen below the cave cut-off (dark, may simply not be drawn), air above it.
//! Observed blocks that render identically to the regen block adopt the regen state (kelp age, leaves
//! distance…): see `adopt_invisible`.

use std::collections::HashMap;

use anyhow::Result;
use bmr_invert::evidence::Evidence;
use bmr_invert::face::{Cell, Liquid};
use bmr_world::{BlockState, Chunk, ChunkPos, World};

use crate::columns::{Fill, Gap};
use crate::material::Segment;

pub struct RegenWorld {
    chunks: HashMap<ChunkPos, Chunk>,
}

impl RegenWorld {
    pub fn load(world: &World) -> Result<Self> {
        let mut chunks = HashMap::new();
        for r in world.regions()? {
            chunks.extend(world.read_region(r)?.into_iter().filter(|(_, c)| c.is_full()));
        }
        Ok(Self { chunks })
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&pos)
    }

    /// `None` = air or not generated.
    pub fn block(&self, (x, y, z): Cell) -> Option<&BlockState> {
        let c = self.chunks.get(&(x.div_euclid(16), z.div_euclid(16)))?;
        c.block(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize).filter(|s| !s.is_air())
    }
}

pub struct Context<'a> {
    pub regen: &'a RegenWorld,
    pub evidence: &'a Evidence,
    pub is_full: &'a dyn Fn(&BlockState) -> bool,
    /// Material when evidence demands a full block the regen does not have.
    pub prior: &'a dyn Fn(Cell) -> BlockState,
    pub liquid: &'a dyn Fn(Liquid) -> BlockState,
    /// BlueMap `remove-caves-below-y`: below it, dark cells may be unrendered.
    pub cave_y: i32,
}

pub fn fill_gap(g: &Gap, cx: &Context) -> Vec<Segment> {
    let (x, z) = g.column;
    let mut out: Vec<Segment> = Vec::new();
    for y in (g.ylo..=g.yhi).rev() {
        let Some(state) = cell_state(g, (x, y, z), cx) else { continue };
        match out.last_mut() {
            Some(s) if s.state == state && s.ylo == y + 1 => s.ylo = y,
            _ => out.push(Segment { column: g.column, ylo: y, yhi: y, state }),
        }
    }
    out
}

/// Observed states replaced by the regen state at the same cell when both render identically
/// (`same_render`), i.e. they differ only in properties the map cannot show. Returns the count.
pub fn adopt_invisible(
    blocks: &mut HashMap<Cell, BlockState>,
    regen: &RegenWorld,
    same_render: &dyn Fn(&BlockState, &BlockState) -> bool,
) -> usize {
    let mut n = 0;
    for (&cell, state) in blocks.iter_mut() {
        if let Some(r) = regen.block(cell) {
            if r != state && r.name == state.name && same_render(state, r) {
                *state = r.clone();
                n += 1;
            }
        }
    }
    n
}

fn cell_state(g: &Gap, cell: Cell, cx: &Context) -> Option<BlockState> {
    let r = cx.regen.block(cell);
    let full = r.is_some_and(cx.is_full);
    let ev = cx.evidence;
    if ev.solid.contains(&cell) {
        return Some(if full { r.cloned().unwrap() } else { (cx.prior)(cell) });
    }
    if ev.open.contains(&cell) {
        return if full { None } else { r.cloned() };
    }
    match g.fill {
        Fill::Solid => r.cloned(),
        Fill::Liquid(l) if g.floored => Some((cx.liquid)(l)),
        // regen air deep down is a dark cave under the sea, not missing water
        Fill::Liquid(l) => r.cloned().or_else(|| (cell.1 >= cx.cave_y).then(|| (cx.liquid)(l))),
        Fill::Air if cell.1 < cx.cave_y => r.cloned(),
        Fill::Air => None,
    }
}
