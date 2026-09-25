//! `bmr pull <url>`: the one-command user path — mirror the site, check the pack fits, reconstruct,
//! package as a zip (or folder) and optionally a schematic. Only use on maps you own or may reverse.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use bmr_invert::timings::Timings;
use bmr_pack::{Pack, Verdict};

use crate::fetch::site_slug;
use crate::pack::select_pack;
use crate::reconstruct::{Inputs, reconstruct, report};
use crate::reverse::OptionArgs;

#[derive(clap::Args)]
pub struct Args {
    /// BlueMap web address, e.g. https://map.example.com/
    url: String,
    /// Output: `*.zip` (a world to drop into saves/) or a folder [default: <site>-<map>.zip]
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// Map id when the site has several (the list is printed otherwise)
    #[arg(long)]
    map: Option<String>,
    /// Pack file [default: from ./packs or next to bmr, matching the site's BlueMap version]
    #[arg(long)]
    pack: Option<PathBuf>,
    /// Also export the whole reconstruction as a Sponge .schem
    #[arg(long)]
    schem: Option<PathBuf>,
    /// Download cache (resumable; re-runs download nothing new)
    #[arg(long)]
    cache: Option<PathBuf>,
    /// Parallel downloads
    #[arg(long, default_value_t = 4)]
    concurrency: usize,
    /// Pause before each request per download worker (be gentle with other people's servers)
    #[arg(long, default_value_t = 25)]
    delay_ms: u64,
    /// Same-seed regeneration of the untouched terrain (tools/regen_world.py) for exact underground
    #[arg(long)]
    regen: Option<PathBuf>,
    /// Dimension [default: guessed from the map id: nether / end / overworld]
    #[arg(long)]
    dimension: Option<String>,
    /// Continue even if the pack does not fit the site
    #[arg(long)]
    force: bool,
    #[arg(long)]
    timings: Option<PathBuf>,
    #[command(flatten)]
    opts: OptionArgs,
}

pub fn run(a: Args) -> Result<()> {
    let total = Instant::now();
    let mut t = Timings::default();
    let slug = site_slug(&a.url);
    let cache = a.cache.clone().unwrap_or_else(|| PathBuf::from("work/cache").join(&slug));

    println!("[1/4] mirroring {} → {}", a.url, cache.display());
    let fetch_opts = bmr_fetch::Options {
        base_url: a.url.clone(),
        out: cache.clone(),
        maps: a.map.iter().cloned().collect(),
        concurrency: a.concurrency,
        delay: Duration::from_millis(a.delay_ms),
    };
    let summaries = t.time("fetch", || bmr_fetch::mirror(&fetch_opts))?;
    let map_id = match (&a.map, summaries.as_slice()) {
        (Some(m), _) => m.clone(),
        (None, [only]) => only.id.clone(),
        (None, many) => {
            let ids: Vec<&str> = many.iter().map(|s| s.id.as_str()).collect();
            bail!("the site has several maps {ids:?}: pick one with --map (all were mirrored)");
        }
    };
    let map = bmr_fetch::LocalMap::open(&cache, Some(&map_id))?;
    println!("      map `{}` ({}), {} hires tiles", map_id, map.settings.name, map.tiles(0).len());

    println!("[2/4] choosing a pack (texture fingerprint; site runs BlueMap {})", map.bluemap_version.as_deref().unwrap_or("?"));
    let site_textures = bmr_prbm::parse_texture_names(&map.textures_json()?)?;
    let (pack_path, ranking) = select_pack(a.pack.as_deref(), map.bluemap_version.as_deref(), &site_textures)?;
    for (i, line) in ranking.iter().enumerate() {
        println!("      {} {line}", if i == 0 { "→" } else { " " });
    }
    let pack = t.time("pack_load", || Pack::load(&pack_path))?;
    let compat = bmr_pack::check(&pack, map.bluemap_version.as_deref(), &site_textures);
    println!("      pack {} (Minecraft {}, BlueMap {})", pack_path.display(), pack.meta.mc_version, pack.meta.bluemap_version);
    for line in compat.explain() {
        println!("      {line}");
    }
    match compat.verdict() {
        Verdict::Fail if !a.force => bail!("this pack does not fit the site (see above); use a matching pack or --force"),
        Verdict::Fail | Verdict::Warn => println!("      continuing: blocks with unknown textures will be left out"),
        Verdict::Ok => {}
    }

    let (dimension, heights) = dimension_for(&map_id, a.dimension.as_deref());
    let mut opts = a.opts.options();
    if a.dimension.is_none() && heights != (opts.min_y, opts.max_y) {
        (opts.min_y, opts.max_y) = heights;
    }
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from(format!("{slug}-{map_id}.zip")));
    let zip = out.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    let world_dir = if zip { cache.join("reconstructed").join(&map_id).join("world") } else { out.clone() };
    if zip {
        let _ = std::fs::remove_dir_all(world_dir.parent().context("world dir")?);
    }

    println!("[3/4] reconstructing ({dimension}, y {}..{})", opts.min_y, opts.max_y);
    let regen = a.regen.as_ref().map(|p| bmr_world::World::open(p, &dimension, Some(pack.registry.clone()))).transpose()?;
    let inputs = Inputs {
        map: &map,
        lib: &pack.library,
        registry: &pack.registry,
        template: &pack.template,
        style: pack.palette_style(),
        dimension: &dimension,
        regen: regen.as_ref(),
        out: &world_dir,
        opts: &opts,
    };
    let (totals, wt) = reconstruct(&inputs)?;
    t.accumulate(wt);
    let unmatched_pct = 100.0 * totals.unmatched as f64 / totals.cells.max(1) as f64;
    println!(
        "      {} blocks recognised, {:.2}% unrecognised, {} unseen solid + {} unseen liquid filled, {} chunks",
        totals.cells - totals.unmatched, unmatched_pct, totals.solid, totals.liquid, totals.chunks
    );
    if unmatched_pct > 1.0 {
        println!("      note: >1% unrecognised usually means custom models (resource pack/mods) — see the texture sets above");
    }

    println!("[4/4] writing output");
    if zip {
        let files = t.time("zip", || bmr_world::zip_world(&world_dir, &out))?;
        println!("      {} ({files} files, {:.1} MB) — extract into your saves/ folder", out.display(), size_mb(&out));
    } else {
        println!("      world folder {}", out.display());
    }
    if let Some(schem) = &a.schem {
        let world = bmr_world::World::open(&world_dir, &dimension, Some(pack.registry.clone()))?;
        let area = bmr_world::Area { min: [i32::MIN / 4, opts.min_y, i32::MIN / 4], max: [i32::MAX / 4, opts.max_y, i32::MAX / 4] };
        let extent = region_extent(&world, area)?;
        let s = t.time("schem", || bmr_world::export_schem(&world, extent, true, &map_id, schem))?;
        let size: [i32; 3] = std::array::from_fn(|i| s.area.max[i] - s.area.min[i] + 1);
        println!("      {} ({}x{}x{}, {:.1} MB)", schem.display(), size[0], size[1], size[2], size_mb(schem));
    }
    t.record("total", total.elapsed());
    println!("done in {:.1?}", total.elapsed());
    if a.timings.is_some() {
        report(&t, a.timings.as_deref())?;
    }
    Ok(())
}

/// (dimension id, (min_y, max_y)) from an explicit value or the map id.
fn dimension_for(map_id: &str, explicit: Option<&str>) -> (String, (i32, i32)) {
    let id = explicit.map_or_else(|| map_id.to_ascii_lowercase(), str::to_ascii_lowercase);
    if id.contains("nether") {
        ("minecraft:the_nether".into(), (0, 255))
    } else if id.contains("end") {
        ("minecraft:the_end".into(), (0, 255))
    } else {
        ("minecraft:overworld".into(), (-64, 319))
    }
}

/// Block box of the world's region files, clamped to `area`'s heights.
fn region_extent(world: &bmr_world::World, area: bmr_world::Area) -> Result<bmr_world::Area> {
    let r = world.regions()?;
    let (x0, x1) = (r.iter().map(|r| r.0).min().context("no regions")?, r.iter().map(|r| r.0).max().unwrap());
    let (z0, z1) = (r.iter().map(|r| r.1).min().unwrap(), r.iter().map(|r| r.1).max().unwrap());
    Ok(bmr_world::Area { min: [x0 * 512, area.min[1], z0 * 512], max: [x1 * 512 + 511, area.max[1], z1 * 512 + 511] })
}

fn size_mb(p: &Path) -> f64 {
    std::fs::metadata(p).map_or(0.0, |m| m.len() as f64 / 1e6)
}
