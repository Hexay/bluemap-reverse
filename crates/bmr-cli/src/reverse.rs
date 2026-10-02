//! `bmr reverse`: reconstruct from a local mirror with the library built from local debug-world files
//! (development path; users run `bmr pull` with a pack).

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use bmr_fill::Profile;
use bmr_invert::Library;
use bmr_invert::timings::Timings;

use crate::reconstruct::{Inputs, Options, reconstruct, report};
use crate::ui::progress;
use crate::{LibraryArgs, MirrorArgs, WorldArgs};

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    mirror: MirrorArgs,
    /// Output world dir (must not exist)
    #[arg(short, long)]
    out: PathBuf,
    #[command(flatten)]
    library: LibraryArgs,
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
    /// Lowest block y [default: the dimension's]
    #[arg(long, allow_hyphen_values = true)]
    min_y: Option<i32>,
    /// Highest block y [default: the dimension's]
    #[arg(long, allow_hyphen_values = true)]
    max_y: Option<i32>,
    /// The map's BlueMap `remove-caves-below-y`, not published by the site [default: BlueMap's for the
    /// dimension: 55 overworld, none nether/end]
    #[arg(long, allow_hyphen_values = true)]
    cave_y: Option<i32>,
    /// Heights the map's `render-mask` leaves out, `lo,hi` or `none` [default: BlueMap's for the dimension:
    /// 90,127 in the nether]
    #[arg(long)]
    mask_y: Option<String>,
    /// Print this many most frequent unmatched texture sets per window
    #[arg(long, default_value_t = 5)]
    show_unmatched: usize,
}

impl OptionArgs {
    /// BlueMap's default map for `dimension`, with any heights given on the command line.
    pub fn options(&self, dimension: &str) -> Result<Options> {
        let mut profile = Profile::for_dimension(dimension);
        profile.min_y = self.min_y.unwrap_or(profile.min_y);
        profile.max_y = self.max_y.unwrap_or(profile.max_y);
        profile.cave_y = self.cave_y.unwrap_or(profile.cave_y);
        if let Some(m) = &self.mask_y {
            profile.mask = match m.as_str() {
                "none" => None,
                _ => {
                    let (lo, hi) = m.split_once(',').context("--mask-y takes lo,hi or none")?;
                    Some((lo.trim().parse()?, hi.trim().parse()?))
                }
            };
        }
        Ok(Options {
            no_fill: self.no_fill,
            no_window: self.no_window,
            halo: self.halo,
            profile,
            show_unmatched: self.show_unmatched,
        })
    }
}

pub fn run(a: Args) -> Result<()> {
    let total = Instant::now();
    let mut t = Timings::default();
    let registry = t.time("registry", || a.world_args.registry())?.context("reverse needs the block registry")?;
    let (lib_map, lib_world) = a.library.library_sources(&registry)?;
    let style = lib_world.palette_style()?.context("library world has no palettes")?;
    let lib = t.time("library", || Library::build(&lib_map, &lib_world, &registry))?;
    t.extend("library", lib.stats.timings.clone());
    progress!("library: {} states ({} with overhanging geometry)", lib.stats.states, lib.stats.overhang_states);
    let biome_tints = t.time("biome_tints", || match a.library.biomes_or_warn(&registry) {
        Some((m, w)) => bmr_invert::tints::learn(&m, &w),
        None => Ok(Vec::new()),
    })?;

    let map = a.mirror.open()?;
    let regen = a.regen.as_ref().map(|p| a.world_args.open(p, &Some(registry.clone()))).transpose()?;
    let template = bmr_world::read_template(&a.library.template)?;
    let opts = a.opts.options(&a.world_args.dimension)?;
    let inputs = Inputs {
        map: &map,
        lib: &lib,
        biome_tints: &biome_tints,
        registry: &registry,
        template: &template,
        style,
        dimension: &a.world_args.dimension,
        regen: regen.as_ref(),
        out: &a.out,
        extend: false,
        opts: &opts,
    };
    let (totals, wt) = reconstruct(&inputs)?;
    t.accumulate(wt);
    println!(
        "cells {} (unmatched {}), unseen solid {}, unseen liquid {}, adopted from regen {} → {} chunks in {}{}",
        totals.cells,
        totals.unmatched,
        totals.solid,
        totals.liquid,
        totals.adopted,
        totals.chunks,
        a.out.display(),
        totals.overlap_note()
    );
    if let Some(zip) = &a.zip {
        let files = t.time("zip", || bmr_world::zip_world(&a.out, zip))?;
        println!("{files} files → {} ({} MB)", zip.display(), std::fs::metadata(zip)?.len() >> 20);
    }
    t.record("total", total.elapsed());
    report(&t, a.timings.as_deref())
}
