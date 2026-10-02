//! Unseen cells from a same-seed regeneration of the untouched terrain, decided per cell against face
//! evidence (gap-level classification is too coarse: one gap can hold a lit cavity above dark rock):
//! - solid evidence: regen if it is a full block there, else the height prior
//! - open evidence: air, unless regen has something non-full (water, plants)
//! - no evidence: solid gap → regen (above the cave cut-off only full blocks); liquid gap with an observed floor → the liquid (a player pool must
//!   not get regen dirt back), liquid gap running to the world floor → regen (real deep ocean floors,
//!   dark caves below them), liquid only where regen is air above the cave cut-off;
//!   air gap → regen below the cave cut-off (dark, may simply not be drawn), air above it.
//!
//! Observed blocks that render identically to the regen block adopt the regen state (kelp age, leaves
//! distance…): see `adopt_invisible`.

use anyhow::Result;
use bmr_invert::face::{Cell, Liquid};
use bmr_world::{BlockState, Chunk, ChunkPos, StateId, StateTable, World};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::Segment;
use crate::columns::{Cursor, EvidenceByColumn, Fill, Gap};

struct RegenChunk {
    chunk: Chunk,
    /// Per section (same order as `chunk.sections`): palette index → interned id.
    palette_ids: Vec<Vec<StateId>>,
}

pub struct RegenWorld {
    chunks: FxHashMap<ChunkPos, RegenChunk>,
}

impl RegenWorld {
    /// Chunks within `area` (inclusive chunk-coordinate box; `None` = everything). Regions decode in
    /// parallel; palettes are interned afterwards (few distinct states per section).
    pub fn load(world: &World, table: &mut StateTable, area: Option<(ChunkPos, ChunkPos)>) -> Result<Self> {
        let inside =
            |(x, z): ChunkPos| area.is_none_or(|((x0, z0), (x1, z1))| (x0..=x1).contains(&x) && (z0..=z1).contains(&z));
        let region_hit = |(rx, rz): (i32, i32)| {
            area.is_none_or(|((x0, z0), (x1, z1))| {
                rx * 32 <= x1 && rx * 32 + 31 >= x0 && rz * 32 <= z1 && rz * 32 + 31 >= z0
            })
        };
        let regions: Vec<_> = world.regions()?.into_iter().filter(|&r| region_hit(r)).collect();
        // decode only the chunks in the area: windows overlap regions, whole-region decodes repeat work
        let decoded = regions.par_iter().map(|&r| world.read_region_where(r, &inside)).collect::<Result<Vec<_>>>()?;
        let mut chunks = FxHashMap::default();
        for (pos, chunk) in decoded.into_iter().flatten().filter(|(_, c)| c.is_full()) {
            let palette_ids =
                chunk.sections.iter().map(|s| s.palette.iter().map(|b| table.intern(b)).collect()).collect();
            chunks.insert(pos, RegenChunk { chunk, palette_ids });
        }
        Ok(Self { chunks })
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&pos).map(|c| &c.chunk)
    }

    /// Interned state at a cell, `None` where not generated (air is returned as its id).
    pub fn id(&self, (x, y, z): Cell) -> Option<StateId> {
        self.column(x, z)?.id(y)
    }

    /// One chunk lookup for a whole column (the fill walks columns).
    pub fn column(&self, x: i32, z: i32) -> Option<RegenColumn<'_>> {
        let rc = self.chunks.get(&(x.div_euclid(16), z.div_euclid(16)))?;
        Some(RegenColumn { rc, xz: (z.rem_euclid(16) as usize) * 16 + x.rem_euclid(16) as usize })
    }
}

pub struct RegenColumn<'a> {
    rc: &'a RegenChunk,
    /// `z*16 + x` within the chunk.
    xz: usize,
}

impl RegenColumn<'_> {
    pub fn id(&self, y: i32) -> Option<StateId> {
        let sections = &self.rc.chunk.sections;
        let sy = y.div_euclid(16);
        // sections are sorted and normally contiguous: index directly, search only if there is a hole
        let guess = (sy - sections.first()?.y) as usize;
        let i = match sections.get(guess) {
            Some(s) if s.y == sy => guess,
            _ => sections.binary_search_by_key(&sy, |s| s.y).ok()?,
        };
        let idx = sections[i].index((y.rem_euclid(16) as usize) * 256 + self.xz);
        Some(self.rc.palette_ids[i][idx as usize])
    }
}

pub struct Context<'a> {
    pub regen: &'a RegenWorld,
    pub evidence: &'a EvidenceByColumn,
    /// Indexed by `StateId`.
    pub full: &'a [bool],
    pub air: &'a [bool],
    /// Height prior (when evidence demands a full block the regen does not have).
    pub prior: &'a (dyn Fn(i32) -> StateId + Sync),
    pub liquid: &'a (dyn Fn(Liquid) -> StateId + Sync),
    /// BlueMap `remove-caves-below-y`: below it, dark cells may be unrendered.
    pub cave_y: i32,
}

/// Append the gap's segments to `out` (one per-thread buffer, no allocation per gap).
pub fn fill_gap_into(g: &Gap, cx: &Context, out: &mut Vec<Segment>) {
    let column = cx.regen.column(g.column.0, g.column.1);
    let ev = cx.evidence.get(&g.column);
    let mut solid = Cursor::new(ev.map_or(&[][..], |e| &e.solid));
    let mut open = Cursor::new(ev.map_or(&[][..], |e| &e.open));
    let start = out.len();
    for y in (g.ylo..=g.yhi).rev() {
        let r = column.as_ref().and_then(|c| c.id(y)).filter(|id| !cx.air[id.0 as usize]);
        let Some(state) = cell_state(g, y, r, solid.hit(y), open.hit(y), cx) else { continue };
        match out[start..].last_mut() {
            Some(s) if s.state == state && s.ylo == y + 1 => s.ylo = y,
            _ => out.push(Segment { column: g.column, ylo: y, yhi: y, state }),
        }
    }
}

fn cell_state(g: &Gap, y: i32, r: Option<StateId>, solid_ev: bool, open_ev: bool, cx: &Context) -> Option<StateId> {
    let full = r.is_some_and(|id| cx.full[id.0 as usize]);
    if solid_ev {
        return Some(if full { r.unwrap() } else { (cx.prior)(y) });
    }
    if open_ev {
        return if full { None } else { r };
    }
    match g.fill {
        // above the cut-off nothing is culled for darkness: a non-full cell here would have made the solid
        // block above it draw a face, so every cell of the gap is a full block
        Fill::Solid if y >= cx.cave_y && !full => Some((cx.prior)(y)),
        Fill::Solid => r,
        Fill::Liquid(l) if g.floored => Some((cx.liquid)(l)),
        // regen air deep down is a dark cave under the sea, not missing water
        Fill::Liquid(l) => r.or_else(|| (y >= cx.cave_y).then(|| (cx.liquid)(l))),
        Fill::Air if y < cx.cave_y => r,
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
