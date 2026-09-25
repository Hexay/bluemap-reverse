use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use bmr_fill::Bounds;
use bmr_invert::Library;
use bmr_invert::timings::Timings;
use bmr_world::{BlockRegistry, BlockState, Chunk, ChunkBuilder, ChunkLayout, StateId, StateTable, WorldWriter};

use crate::copy_world::DEFAULT_TEMPLATE;
use crate::window::{self, Window};
use crate::{MirrorArgs, WorldArgs};

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    mirror: MirrorArgs,
    /// Output world dir (must not exist)
    out: PathBuf,
    /// Mirror of BlueMap's render of the debug world (signature source)
    #[arg(long, default_value = "work/cache/debug")]
    library_mirror: PathBuf,
    /// The debug world itself (ground-truth states for the library)
    #[arg(long, default_value = "work/worlds/debug/world")]
    library_world: PathBuf,
    #[arg(long, default_value = DEFAULT_TEMPLATE)]
    template: PathBuf,
    /// Only place what the tiles show (skip hidden-volume fill and game rules)
    #[arg(long)]
    no_fill: bool,
    /// Same-seed regeneration of the untouched terrain (tools/regen_world.py) for unseen cells + biomes
    #[arg(long)]
    regen: Option<PathBuf>,
    /// Process the whole map at once instead of one region window at a time (memory grows with the map)
    #[arg(long)]
    no_window: bool,
    /// Halo around each region window, in blocks
    #[arg(long, default_value_t = 32)]
    halo: i32,
    /// Dimension build height (overworld default)
    #[arg(long, default_value_t = -64, allow_hyphen_values = true)]
    min_y: i32,
    #[arg(long, default_value_t = 319)]
    max_y: i32,
    /// The map's BlueMap `remove-caves-below-y` (not published by the site)
    #[arg(long, default_value_t = 55, allow_hyphen_values = true)]
    cave_y: i32,
    #[arg(long, default_value_t = 10)]
    show_unmatched: usize,
    /// Write per-stage timings as JSON {stage: seconds}
    #[arg(long)]
    timings: Option<PathBuf>,
    #[command(flatten)]
    world_args: WorldArgs,
}

/// Map-wide sums over windows (window counts include their halo cells).
#[derive(Default)]
struct Totals {
    cells: usize,
    unmatched: usize,
    solid: usize,
    liquid: usize,
    adopted: usize,
    chunks: usize,
}

pub fn run(a: Args) -> Result<()> {
    let total = Instant::now();
    let mut t = Timings::default();
    let registry = t.time("registry", || a.world_args.registry())?.context("reverse needs the block registry")?;
    let lib = t.time("library", || -> Result<Library> {
        let lib_map = bmr_fetch::LocalMap::open(&a.library_mirror, None)?;
        let lib_world = a.world_args.open(&a.library_world, &Some(registry.clone()))?;
        Library::build(&lib_map, &lib_world, &registry)
    })?;
    t.extend("library", lib.stats.timings.clone());
    println!("library: {} states ({} with overhanging geometry)", lib.stats.states, lib.stats.overhang_states);

    let map = a.mirror.open()?;
    let textures = bmr_invert::map_textures(&map)?;
    let windows =if a.no_window { window::whole_map(&map) } else { window::per_region(&map, a.halo) };
    let regen_world = a.regen.as_ref().map(|p| a.world_args.open(p, &Some(registry.clone()))).transpose()?;
    let writer = WorldWriter::create(&a.out, &a.template, &a.world_args.dimension, registry.clone())?;
    let mut table = StateTable::default();
    let air = table.intern(&BlockState::new("minecraft:air".into(), Vec::new()));

    let mut totals = Totals::default();
    let mut per_window = Timings::default();
    for (i, win) in windows.iter().enumerate() {
        let ctx = Ctx { a: &a, lib: &lib, registry: &registry, map: &map, textures: &textures, regen_world: regen_world.as_ref(), air };
        let (chunks, wt) = run_window(win, &ctx, &mut table, &mut totals)?;
        totals.chunks += chunks.len();
        let mut wt = wt;
        wt.time("write", || writer.write_chunks(chunks))?;
        per_window.accumulate(wt);
        if windows.len() > 1 {
            println!("window {}/{} {:?}: {} tiles, {} columns", i + 1, windows.len(), win.region, win.tiles.len(), win.columns.len());
        }
    }
    println!(
        "cells {} (unmatched {}), unseen solid {}, unseen liquid {}, adopted from regen {} → {} chunks in {}",
        totals.cells, totals.unmatched, totals.solid, totals.liquid, totals.adopted, totals.chunks, a.out.display()
    );
    t.accumulate(per_window);
    t.record("total", total.elapsed());
    report(&t, a.timings.as_ref())
}

struct Ctx<'a> {
    a: &'a Args,
    lib: &'a Library,
    registry: &'a BlockRegistry,
    map: &'a bmr_fetch::LocalMap,
    textures: &'a [bmr_invert::face::Tex],
    regen_world: Option<&'a bmr_world::World>,
    air: StateId,
}

/// Invert, fill and build one window; returns only the window's own chunks.
fn run_window(win: &Window, cx: &Ctx, table: &mut StateTable, totals: &mut Totals) -> Result<(Vec<Chunk>, Timings)> {
    let a = cx.a;
    let mut t = Timings::default();
    let inv = t.time("invert", || bmr_invert::reverse(cx.map, cx.lib, &win.tiles, cx.textures))?;
    totals.cells += inv.stats.cells;
    totals.unmatched += inv.stats.unmatched;
    for (textures, n) in inv.stats.unmatched_textures.iter().take(a.show_unmatched) {
        println!("  unmatched {n:>6}  {textures}");
    }
    let regen = t.time("regen_load", || -> Result<_> {
        cx.regen_world.map(|w| bmr_fill::RegenWorld::load(w, table, win.chunk_area)).transpose()
    })?;

    let mut builder = ChunkBuilder::new(
        ChunkLayout {
            data_version: cx.lib.data_version,
            sections: (a.min_y.div_euclid(16), a.max_y.div_euclid(16)),
            biome: "minecraft:plains".into(),
        },
        cx.air,
    );
    let emits = |(x, _, z): (i32, i32, i32)| win.emits(x, z);
    let mut inner = Timings::default();
    if a.no_fill {
        for (&c, &e) in inv.blocks.iter().filter(|(c, _)| emits(**c)) {
            builder.set_block(c, table.intern(&cx.lib.entries[e].state));
        }
    } else {
        // regen decides each cell on its own; only the prior fill searches neighbouring columns
        let columns = if regen.is_some() { win.columns.clone() } else { win.halo_columns.clone() };
        let bounds = Bounds { columns, min_y: a.min_y, max_y: a.max_y, cave_y: a.cave_y };
        let filled =
            t.time("fill", || bmr_fill::complete(&inv, cx.lib, cx.registry, &bounds, regen.as_ref(), table));
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
fn report(t: &Timings, json: Option<&PathBuf>) -> Result<()> {
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
