//! Unseen cells from a same-seed regeneration of the untouched terrain, decided per cell against face
//! evidence (gap-level classification is too coarse: one gap can hold a lit cavity above dark rock):
//! - solid evidence: regen if it is a full block there, else the height prior
//! - open evidence: air, unless regen has something non-full (water, plants)
//! - no evidence: solid gap → regen (above the cave cut-off only full blocks); liquid gap with an observed floor → the liquid (a player pool must
//!   not get regen dirt back), liquid gap running to the world floor → regen (real deep ocean floors,
//!   dark caves below them), liquid only where regen is air above the cave cut-off;
//!   air gap → regen below the cave cut-off (dark, may simply not be drawn), air above it.
//! Observed blocks that render identically to the regen block adopt the regen state (kelp age, leaves
//! distance…): see `adopt_invisible`.

use anyhow::Result;
use rustc_hash::FxHashMap;
use bmr_invert::evidence::Evidence;
use bmr_invert::face::{Cell, Liquid};
use bmr_world::{BlockState, Chunk, ChunkPos, StateId, StateTable, World};
use rayon::prelude::*;

use crate::Segment;
use crate::columns::{Fill, Gap};

struct RegenChunk {
    chunk: Chunk,
    /// Per section (same order as `chunk.sections`): palette index → interned id.
    palette_ids: Vec<Vec<StateId>>,
}

pub struct RegenWorld {
    chunks: FxHashMap<ChunkPos, RegenChunk>,
}

impl RegenWorld {
    /// Regions decode in parallel; palettes are interned afterwards (few distinct states per section).
    pub fn load(world: &World, table: &mut StateTable) -> Result<Self> {
        let regions = world.regions()?;
        let decoded = regions.par_iter().map(|&r| world.read_region(r)).collect::<Result<Vec<_>>>()?;
        let mut chunks = FxHashMap::default();
        for (pos, chunk) in decoded.into_iter().flatten().filter(|(_, c)| c.is_full()) {
            let palette_ids = chunk.sections.iter().map(|s| s.palette.iter().map(|b| table.intern(b)).collect()).collect();
            chunks.insert(pos, RegenChunk { chunk, palette_ids });
        }
        Ok(Self { chunks })
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&pos).map(|c| &c.chunk)
    }

    /// Interned state at a cell, `None` where not generated (air is returned as its id).
    pub fn id(&self, (x, y, z): Cell) -> Option<StateId> {
        let rc = self.chunks.get(&(x.div_euclid(16), z.div_euclid(16)))?;
        let i = rc.chunk.sections.binary_search_by_key(&y.div_euclid(16), |s| s.y).ok()?;
        let s = &rc.chunk.sections[i];
        let idx = s.index(((y.rem_euclid(16) as usize) * 16 + z.rem_euclid(16) as usize) * 16 + x.rem_euclid(16) as usize);
        Some(rc.palette_ids[i][idx as usize])
    }
}

pub struct Context<'a> {
    pub regen: &'a RegenWorld,
    pub evidence: &'a Evidence,
    /// Indexed by `StateId`.
    pub full: &'a [bool],
    pub air: &'a [bool],
    /// Height prior (when evidence demands a full block the regen does not have).
    pub prior: &'a (dyn Fn(i32) -> StateId + Sync),
    pub liquid: &'a (dyn Fn(Liquid) -> StateId + Sync),
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

fn cell_state(g: &Gap, cell: Cell, cx: &Context) -> Option<StateId> {
    let r = cx.regen.id(cell).filter(|id| !cx.air[id.0 as usize]);
    let full = r.is_some_and(|id| cx.full[id.0 as usize]);
    let ev = cx.evidence;
    if ev.solid.contains(&cell) {
        return Some(if full { r.unwrap() } else { (cx.prior)(cell.1) });
    }
    if ev.open.contains(&cell) {
        return if full { None } else { r };
    }
    match g.fill {
        // above the cut-off nothing is culled for darkness: a non-full cell here would have made the solid
        // block above it draw a face, so every cell of the gap is a full block
        Fill::Solid if cell.1 >= cx.cave_y && !full => Some((cx.prior)(cell.1)),
        Fill::Solid => r,
        Fill::Liquid(l) if g.floored => Some((cx.liquid)(l)),
        // regen air deep down is a dark cave under the sea, not missing water
        Fill::Liquid(l) => r.or_else(|| (cell.1 >= cx.cave_y).then(|| (cx.liquid)(l))),
        Fill::Air if cell.1 < cx.cave_y => r,
        Fill::Air => None,
    }
}

/// Observed states replaced by the regen state at the same cell when both render identically
/// (`same_render`), i.e. they differ only in properties the map cannot show. Returns the count.
pub fn adopt_invisible(
    blocks: &mut FxHashMap<Cell, BlockState>,
    regen: &RegenWorld,
    table: &StateTable,
    same_render: &dyn Fn(&BlockState, &BlockState) -> bool,
) -> usize {
    let mut n = 0;
    for (&cell, state) in blocks.iter_mut() {
        let Some(r) = regen.id(cell).map(|id| table.get(id)) else { continue };
        if r != state && r.name == state.name && same_render(state, r) {
            *state = r.clone();
            n += 1;
        }
    }
    n
}
