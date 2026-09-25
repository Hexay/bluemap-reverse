use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use bmr_fill::Bounds;
use bmr_invert::Library;
use bmr_world::{BlockState, ChunkLayout, WorldWriter, chunks_from_blocks};

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
    println!(
        "cells {}: {:?}, overhang-only {}, unmatched {}, occluder evidence {} in {:.1?}",
        s.cells, s.by_how, s.overhang_cells, s.unmatched, inv.occluders.len(), t.elapsed()
    );
    for (textures, n) in s.unmatched_textures.iter().take(a.show_unmatched) {
        println!("  unmatched {n:>6}  {textures}");
    }

    let blocks: Vec<((i32, i32, i32), BlockState)> = if a.no_fill {
        inv.blocks.iter().map(|(&c, &e)| (c, lib.entries[e].state.clone())).collect()
    } else {
        let grid = map.settings.hires_grid();
        let rendered: HashSet<_> = map.tiles(0).into_iter().collect();
        let column = move |x: i32, z: i32| rendered.contains(&grid.tile_of(x, z));
        let bounds = Bounds { column: &column, min_y: a.min_y, max_y: a.max_y };
        let (filled, fs) = bmr_fill::complete(&inv, &lib, &registry, &bounds);
        println!("fill: {} observed + {} hidden solid, {} leaves adjusted", fs.observed, fs.hidden_solid, fs.leaves_adjusted);
        filled.into_iter().collect()
    };

    let layout = ChunkLayout {
        data_version: lib.data_version,
        sections: (a.min_y.div_euclid(16), a.max_y.div_euclid(16)),
        biome: "minecraft:plains".into(),
    };
    let chunks = chunks_from_blocks(blocks.iter().map(|(c, s)| (*c, s)), &layout);
    let writer = WorldWriter::create(&a.out, &a.template, &a.world_args.dimension, registry)?;
    let n = chunks.len();
    writer.write_chunks(chunks)?;
    println!("{n} chunks → {}", a.out.display());
    Ok(())
}
