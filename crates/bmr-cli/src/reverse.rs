use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use bmr_fill::Bounds;
use bmr_invert::Library;
use bmr_invert::timings::Timings;
use bmr_world::{BlockState, ChunkBuilder, ChunkLayout, StateTable, WorldWriter};

use crate::copy_world::DEFAULT_TEMPLATE;
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

pub fn run(a: Args) -> Result<()> {
    let total = Instant::now();
    let mut t = Timings::default();
    let registry = t.time("registry", || a.world_args.registry())?.context("reverse needs the block registry")?;
    let lib = t.time("library", || -> Result<Library> {
        let lib_map = bmr_fetch::LocalMap::open(&a.library_mirror, None)?;
        let lib_world = a.world_args.open(&a.library_world, &Some(registry.clone()))?;
        Library::build(&lib_map, &lib_world, &registry)
    })?;
    println!("library: {} states ({} with overhanging geometry)", lib.stats.states, lib.stats.overhang_states);

    let map = a.mirror.open()?;
    let inv = t.time("invert", || bmr_invert::reverse(&map, &lib))?;
    let s = &inv.stats;
    let ev = &inv.evidence;
    println!(
        "cells {}: {:?}, liquid {}, waterlogged {}, overhang-only {}, unmatched {} | evidence solid {} liquid {} open {}",
        s.cells, s.by_how, s.liquid_cells, s.waterlogged, s.overhang_cells, s.unmatched,
        ev.solid.len(), ev.liquid.len(), ev.open.len()
    );
    for (textures, n) in s.unmatched_textures.iter().take(a.show_unmatched) {
        println!("  unmatched {n:>6}  {textures}");
    }

    let mut table = StateTable::default();
    let air = table.intern(&BlockState::new("minecraft:air".into(), Vec::new()));
    let regen = t.time("regen_load", || -> Result<_> {
        a.regen
            .as_ref()
            .map(|p| bmr_fill::RegenWorld::load(&a.world_args.open(p, &Some(registry.clone()))?, &mut table))
            .transpose()
    })?;
    let mut builder = ChunkBuilder::new(
        ChunkLayout {
            data_version: lib.data_version,
            sections: (a.min_y.div_euclid(16), a.max_y.div_euclid(16)),
            biome: "minecraft:plains".into(),
        },
        air,
    );
    let mut fill_timings = None;
    if a.no_fill {
        for (&c, &e) in &inv.blocks {
            builder.set_block(c, table.intern(&lib.entries[e].state));
        }
    } else {
        let bounds = Bounds { columns: rendered_columns(&map), min_y: a.min_y, max_y: a.max_y, cave_y: a.cave_y };
        let filled =
            t.time("fill", || bmr_fill::complete(&inv, &lib, &registry, &bounds, regen.as_ref(), &mut table));
        let fs = &filled.stats;
        println!(
            "fill: {} observed, {} unseen solid, {} unseen liquid, {} leaves adjusted, {} adopted from regen",
            fs.observed, fs.solid_cells, fs.liquid_cells, fs.leaves_adjusted, fs.adopted_from_regen
        );
        t.time("build_chunks", || {
            for seg in &filled.segments {
                builder.fill_column(seg.column, seg.ylo, seg.yhi, seg.state);
            }
            for (&c, s) in &filled.blocks {
                builder.set_block(c, table.intern(s));
            }
        });
        fill_timings = Some(filled.stats.timings.clone());
    }
    let mut chunks = t.time("finish_chunks", || builder.finish(&table));
    if let Some(r) = &regen {
        for c in &mut chunks {
            if let Some(src) = r.chunk((c.x, c.z)) {
                c.copy_biomes_from(src);
            }
        }
    }
    let n = chunks.len();
    t.time("write", || -> Result<()> {
        let writer = WorldWriter::create(&a.out, &a.template, &a.world_args.dimension, registry.clone())?;
        writer.write_chunks(chunks)?;
        Ok(())
    })?;
    println!("{n} chunks → {}", a.out.display());

    t.extend("library", lib.stats.timings.clone());
    t.extend("invert", inv.stats.timings.clone());
    if let Some(ft) = fill_timings {
        t.extend("fill", ft);
    }
    t.record("total", total.elapsed());
    report(&t, a.timings.as_ref())
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

/// Every world column inside a rendered hires tile.
fn rendered_columns(map: &bmr_fetch::LocalMap) -> Vec<(i32, i32)> {
    let [w, h] = map.settings.hires.tile_size;
    map.tiles(0)
        .into_iter()
        .flat_map(|t| {
            let [x0, z0] = map.hires_origin(t);
            (x0..x0 + w).flat_map(move |x| (z0..z0 + h).map(move |z| (x, z)))
        })
        .collect()
}
