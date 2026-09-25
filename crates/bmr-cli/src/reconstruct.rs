//! The reconstruction pipeline shared by `bmr reverse` (library from local files) and `bmr pull`
//! (library from a pack): windowed invert → fill → build → write.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use bmr_fetch::LocalMap;
use bmr_fill::Bounds;
use bmr_invert::Library;
use bmr_invert::face::Tex;
use bmr_invert::timings::Timings;
use bmr_world::{
    BlockRegistry, BlockState, Chunk, ChunkBuilder, ChunkLayout, PaletteStyle, StateId, StateTable, World, WorldWriter,
};

use crate::window::{self, Window};

#[derive(Clone)]
pub struct Options {
    /// Only place what the tiles show.
    pub no_fill: bool,
    /// Whole map in one pass (memory grows with the map).
    pub no_window: bool,
    pub halo: i32,
    pub min_y: i32,
    pub max_y: i32,
    /// The map's BlueMap `remove-caves-below-y`.
    pub cave_y: i32,
    pub show_unmatched: usize,
}

pub struct Inputs<'a> {
    pub map: &'a LocalMap,
    pub lib: &'a Library,
    pub registry: &'a Arc<BlockRegistry>,
    pub template: &'a [(String, Vec<u8>)],
    /// Palette encoding of the target Minecraft version.
    pub style: PaletteStyle,
    pub dimension: &'a str,
    pub regen: Option<&'a World>,
    pub out: &'a Path,
    pub opts: &'a Options,
}

/// Map-wide sums over windows (window counts include their halo cells).
#[derive(Default)]
pub struct Totals {
    pub cells: usize,
    pub unmatched: usize,
    pub solid: usize,
    pub liquid: usize,
    pub adopted: usize,
    pub chunks: usize,
}

pub fn reconstruct(inp: &Inputs) -> Result<(Totals, Timings)> {
    let textures = bmr_invert::map_textures(inp.map)?;
    let windows = if inp.opts.no_window { window::whole_map(inp.map) } else { window::per_region(inp.map, inp.opts.halo) };
    let writer = WorldWriter::create(inp.out, inp.template, inp.dimension, inp.registry.clone(), inp.style)?;
    let mut table = StateTable::default();
    let air = table.intern(&BlockState::new("minecraft:air".into(), Vec::new()));

    let mut totals = Totals::default();
    let mut t = Timings::default();
    for (i, win) in windows.iter().enumerate() {
        let (chunks, mut wt) = run_window(win, inp, &textures, air, &mut table, &mut totals)?;
        totals.chunks += chunks.len();
        wt.time("write", || writer.write_chunks(chunks))?;
        t.accumulate(wt);
        if windows.len() > 1 {
            println!("  window {}/{} {:?}: {} tiles, {} columns", i + 1, windows.len(), win.region, win.tiles.len(), win.columns.len());
        }
    }
    Ok((totals, t))
}

/// Invert, fill and build one window; returns only the window's own chunks.
fn run_window(
    win: &Window,
    inp: &Inputs,
    textures: &[Tex],
    air: StateId,
    table: &mut StateTable,
    totals: &mut Totals,
) -> Result<(Vec<Chunk>, Timings)> {
    let o = inp.opts;
    let mut t = Timings::default();
    let inv = t.time("invert", || bmr_invert::reverse(inp.map, inp.lib, &win.tiles, textures))?;
    totals.cells += inv.stats.cells;
    totals.unmatched += inv.stats.unmatched;
    for (textures, n) in inv.stats.unmatched_textures.iter().take(o.show_unmatched) {
        println!("  unmatched {n:>6}  {textures}");
    }
    let regen = t.time("regen_load", || -> Result<_> {
        inp.regen.map(|w| bmr_fill::RegenWorld::load(w, table, win.chunk_area)).transpose()
    })?;

    let layout = ChunkLayout {
        data_version: inp.lib.data_version,
        sections: (o.min_y.div_euclid(16), o.max_y.div_euclid(16)),
        biome: "minecraft:plains".into(),
    };
    let mut builder = ChunkBuilder::new(layout, air);
    let emits = |(x, _, z): (i32, i32, i32)| win.emits(x, z);
    let mut inner = Timings::default();
    if o.no_fill {
        for (&c, &e) in inv.blocks.iter().filter(|(c, _)| emits(**c)) {
            builder.set_block(c, table.intern(&inp.lib.entries[e].state));
        }
    } else {
        // regen decides each cell on its own; only the prior fill searches neighbouring columns
        let columns = if regen.is_some() { win.columns.clone() } else { win.halo_columns.clone() };
        let bounds = Bounds { columns, min_y: o.min_y, max_y: o.max_y, cave_y: o.cave_y };
        let filled =
            t.time("fill", || bmr_fill::complete(&inv, inp.lib, inp.registry, &bounds, regen.as_ref(), table));
        totals.solid += filled.stats.solid_cells;
        totals.liquid += filled.stats.liquid_cells;
        totals.adopted += filled.stats.adopted_from_regen;
        t.time("build_chunks", || {
            for seg in filled.segments.iter().filter(|s| win.emits(s.column.0, s.column.1)) {
                builder.fill_column(seg.column, seg.ylo, seg.yhi, seg.state);
            }
            for (&c, s) in filled.blocks.iter().filter(|(c, _)| emits(**c)) {
                builder.set_block(c, table.intern(s));
            }
        });
        inner.extend("fill", filled.stats.timings.clone());
    }
    let mut chunks = t.time("finish_chunks", || builder.finish(table));
    if let Some(r) = &regen {
        for c in &mut chunks {
            if let Some(src) = r.chunk((c.x, c.z)) {
                c.copy_biomes_from(src);
            }
        }
    }
    t.extend("invert", inv.stats.timings.clone());
    t.accumulate(inner);
    Ok((chunks, t))
}

/// Table on stdout; JSON `{stage: seconds, "<stage>.mb": resident MB}` for tools/bench.py.
pub fn report(t: &Timings, json: Option<&Path>) -> Result<()> {
    for s in &t.0 {
        let indent = if s.name.contains('.') { "    " } else { "  " };
        println!("{indent}{:<22} {:>9.3}s {:>7} MB", s.name, s.duration.as_secs_f64(), s.resident >> 20);
    }
    if let Some(path) = json {
        let mut map = serde_json::Map::new();
        for s in &t.0 {
            map.insert(s.name.clone(), s.duration.as_secs_f64().into());
            map.insert(format!("{}.mb", s.name), ((s.resident >> 20) as f64).into());
        }
        std::fs::write(path, serde_json::to_vec_pretty(&map)?)?;
    }
    Ok(())
}
