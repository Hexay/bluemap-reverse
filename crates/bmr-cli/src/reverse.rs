//! `bmr reverse`: reconstruct from a local mirror with the library built from local debug-world files
//! (development path; users run `bmr pull` with a pack).

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use bmr_invert::Library;
use bmr_invert::timings::Timings;

use crate::copy_world::DEFAULT_TEMPLATE;
use crate::reconstruct::{Inputs, Options, reconstruct, report};
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
    /// Same-seed regeneration of the untouched terrain (tools/regen_world.py) for unseen cells + biomes
    #[arg(long)]
    regen: Option<PathBuf>,
    /// Also package the output world as a zip (extracts to the world folder name; drop into saves/)
    #[arg(long)]
    zip: Option<PathBuf>,
    /// Write per-stage timings as JSON {stage: seconds}
    #[arg(long)]
    timings: Option<PathBuf>,
    #[command(flatten)]
    opts: OptionArgs,
    #[command(flatten)]
    world_args: WorldArgs,
}

/// Pipeline options shared by `reverse` and `pull`.
#[derive(clap::Args)]
pub struct OptionArgs {
    /// Only place what the tiles show (skip hidden-volume fill and game rules)
    #[arg(long)]
    no_fill: bool,
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
    /// Print this many most frequent unmatched texture sets per window
    #[arg(long, default_value_t = 5)]
    show_unmatched: usize,
}

impl OptionArgs {
    pub fn options(&self) -> Options {
        Options {
            no_fill: self.no_fill,
            no_window: self.no_window,
            halo: self.halo,
            min_y: self.min_y,
            max_y: self.max_y,
            cave_y: self.cave_y,
            show_unmatched: self.show_unmatched,
        }
    }
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
    let regen = a.regen.as_ref().map(|p| a.world_args.open(p, &Some(registry.clone()))).transpose()?;
    let template = bmr_world::read_template(&a.template)?;
    let opts = a.opts.options();
    let inputs = Inputs {
        map: &map,
        lib: &lib,
        registry: &registry,
        template: &template,
        dimension: &a.world_args.dimension,
        regen: regen.as_ref(),
        out: &a.out,
        opts: &opts,
    };
    let (totals, wt) = reconstruct(&inputs)?;
    t.accumulate(wt);
    println!(
        "cells {} (unmatched {}), unseen solid {}, unseen liquid {}, adopted from regen {} → {} chunks in {}",
        totals.cells, totals.unmatched, totals.solid, totals.liquid, totals.adopted, totals.chunks, a.out.display()
    );
    if let Some(zip) = &a.zip {
        let files = t.time("zip", || bmr_world::zip_world(&a.out, zip))?;
        println!("{files} files → {} ({} MB)", zip.display(), std::fs::metadata(zip)?.len() >> 20);
    }
    t.record("total", total.elapsed());
    report(&t, a.timings.as_deref())
}
