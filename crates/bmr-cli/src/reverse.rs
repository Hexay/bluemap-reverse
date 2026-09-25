use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use bmr_fill::Bounds;
use bmr_invert::Library;
use bmr_world::{ChunkBuilder, ChunkLayout, WorldWriter};

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
    /// Dimension build height (overworld default)
    #[arg(long, default_value_t = -64, allow_hyphen_values = true)]
    min_y: i32,
    #[arg(long, default_value_t = 319)]
    max_y: i32,
    #[arg(long, default_value_t = 10)]
    show_unmatched: usize,
    #[command(flatten)]
    world_args: WorldArgs,
}

pub fn run(a: Args) -> Result<()> {
    let registry = a.world_args.registry()?.context("reverse needs the block registry")?;
    let t = Instant::now();
    let lib_map = bmr_fetch::LocalMap::open(&a.library_mirror, None)?;
    let lib_world = a.world_args.open(&a.library_world, &Some(registry.clone()))?;
    let lib = Library::build(&lib_map, &lib_world, &registry)?;
    println!("library: {} states ({} with overhanging geometry) in {:.1?}", lib.stats.states, lib.stats.overhang_states, t.elapsed());

    let t = Instant::now();
    let map = a.mirror.open()?;
    let inv = bmr_invert::reverse(&map, &lib)?;
    let s = &inv.stats;
    let ev = &inv.evidence;
    println!(
        "cells {}: {:?}, liquid {}, waterlogged {}, overhang-only {}, unmatched {} | evidence solid {} liquid {} open {} in {:.1?}",
        s.cells, s.by_how, s.liquid_cells, s.waterlogged, s.overhang_cells, s.unmatched,
        ev.solid.len(), ev.liquid.len(), ev.open.len(), t.elapsed()
    );
    for (textures, n) in s.unmatched_textures.iter().take(a.show_unmatched) {
        println!("  unmatched {n:>6}  {textures}");
    }

    let t = Instant::now();
    let mut builder = ChunkBuilder::new(ChunkLayout {
        data_version: lib.data_version,
        sections: (a.min_y.div_euclid(16), a.max_y.div_euclid(16)),
        biome: "minecraft:plains".into(),
    });
    if a.no_fill {
        for (&c, &e) in &inv.blocks {
            builder.set_block(c, &lib.entries[e].state);
        }
    } else {
        let bounds = Bounds { columns: rendered_columns(&map), min_y: a.min_y, max_y: a.max_y };
        let filled = bmr_fill::complete(&inv, &lib, &registry, &bounds);
        let fs = &filled.stats;
        println!(
            "fill: {} observed, {} unseen solid, {} unseen liquid, {} leaves adjusted in {:.1?}",
            fs.observed, fs.solid_cells, fs.liquid_cells, fs.leaves_adjusted, t.elapsed()
        );
        for seg in &filled.segments {
            builder.fill_column(seg.column, seg.ylo, seg.yhi, &seg.state);
        }
        for (&c, s) in &filled.blocks {
            builder.set_block(c, s);
        }
    }
    let chunks = builder.finish();
    let writer = WorldWriter::create(&a.out, &a.template, &a.world_args.dimension, registry)?;
    let n = chunks.len();
    writer.write_chunks(chunks)?;
    println!("{n} chunks → {}", a.out.display());
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
