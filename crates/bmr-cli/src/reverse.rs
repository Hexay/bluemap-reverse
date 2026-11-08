//! `bmr reverse`: reconstruct from a local mirror with the library built from local debug-world files
//! (development path; users run `bmr pull` with a pack).

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use bmr_fetch::LocalMap;
use bmr_fill::{Given, Inferred, Profile};
use bmr_invert::Library;
use bmr_invert::drawn::DrawnHeights;
use bmr_invert::timings::Timings;
use bmr_world::Dimension;

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

/// Blocks of context loaded around each emitted window; `fetch --area` mirrors this far beyond the box.
pub const DEFAULT_HALO: i32 = 32;

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
    #[arg(long, default_value_t = DEFAULT_HALO)]
    pub halo: i32,
    /// Region windows reconstructed at the same time; each adds ~0.25 GB peak memory (structures fixture)
    #[arg(long, default_value_t = 4)]
    window_jobs: usize,
    /// Lowest block y [default: the dimension's, lower if the tiles draw below it]
    #[arg(long, allow_hyphen_values = true)]
    min_y: Option<i32>,
    /// Highest block y [default: the dimension's, higher if the tiles draw above it]
    #[arg(long, allow_hyphen_values = true)]
    max_y: Option<i32>,
    /// The map's BlueMap `remove-caves-below-y`, not published by the site [default: BlueMap's for the
    /// dimension (55 overworld, none nether/end), lower if the tiles draw dark caves below it]
    #[arg(long, allow_hyphen_values = true)]
    cave_y: Option<i32>,
    /// Heights the map's `render-mask` leaves out, `lo,hi` or `none` [default: BlueMap's for the dimension
    /// (90,127 in the nether), shrunk or none if the tiles draw inside it]
    #[arg(long)]
    mask_y: Option<String>,
    /// World seed, a number or text as in server.properties (`bmr seed` finds it): unseen bedrock and
    /// deepslate layers become exact instead of the majority by height
    #[arg(long, allow_hyphen_values = true)]
    seed: Option<String>,
    /// Print this many most frequent unmatched texture sets per window
    #[arg(long, default_value_t = 5)]
    show_unmatched: usize,
}

impl OptionArgs {
    fn given(&self) -> Result<Given> {
        let mask = match self.mask_y.as_deref() {
            None => None,
            Some("none") => Some(None),
            Some(m) => {
                let (lo, hi) = m.split_once(',').context("--mask-y takes lo,hi or none")?;
                Some(Some((lo.trim().parse()?, hi.trim().parse()?)))
            }
        };
        Ok(Given { min_y: self.min_y, max_y: self.max_y, cave_y: self.cave_y, mask })
    }

    /// Options for `map`: render settings not given on the command line are inferred from its drawn faces (one
    /// pass over its hires tiles, see bmr-fill infer.rs) and printed as one line.
    pub fn options_for_map(&self, map: &LocalMap, dimension: &Dimension) -> Result<Options> {
        let given = self.given()?;
        if let Dimension::Other(id) = dimension {
            eprintln!("warning: unknown dimension {id}: assuming overworld render defaults");
        }
        let defaults = Profile::for_dimension(dimension.clone());
        let mut profile = if given.complete() {
            given.apply(defaults)
        } else {
            let inferred = bmr_fill::infer(defaults, &DrawnHeights::scan(map)?, &given);
            println!("{}", settings_line(&inferred));
            inferred.profile
        };
        if let Some(s) = &self.seed {
            let seed = s.parse().unwrap_or_else(|_| i64::from(bmr_seed::java_random::string_hash(s)));
            profile = profile.with_seed(seed);
        }
        Ok(Options {
            no_fill: self.no_fill,
            no_window: self.no_window,
            halo: self.halo,
            area: None,
            window_jobs: self.window_jobs.max(1),
            profile,
            show_unmatched: self.show_unmatched,
        })
    }
}

fn settings_line(i: &Inferred) -> String {
    if i.changed.is_empty() {
        return format!("render settings: {} (the tiles agree with BlueMap's defaults)", i.summary);
    }
    let flags: Vec<String> = i.changed.iter().map(|f| format!("--{f}")).collect();
    format!(
        "render settings: {} — warning: the tiles contradict BlueMap's defaults for {} (set them explicitly to override)",
        i.summary,
        flags.join(", ")
    )
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
    let opts = t.time("settings", || a.opts.options_for_map(&map, &a.world_args.dimension))?;
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
