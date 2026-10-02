//! `bmr pull <url>`: the one-command user path — mirror the site, check the pack fits, reconstruct every
//! map into its dimension of one world, package as a zip (or folder) and optionally a schematic. Only use on
//! maps you own or may reverse.

mod plan;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use bmr_invert::timings::Timings;
use bmr_pack::{Pack, Verdict};

use crate::fetch::DownloadArgs;
use crate::pack_source::{IndexArgs, select_pack};
use crate::paths::site_slug;
use crate::reconstruct::{Inputs, reconstruct, report};
use crate::reverse::OptionArgs;
use crate::schem::world_extent;
use crate::ui::progress;

#[derive(clap::Args)]
pub struct Args {
    /// BlueMap web address, e.g. <https://map.example.com/>
    url: String,
    /// Output: `*.zip` (a world to drop into saves/) or a folder [default: SITE.zip, SITE-MAP.zip with
    /// --map]
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// Only this map [default: every map, each into its dimension of the one world]
    #[arg(long)]
    map: Option<String>,
    /// Pack file [default: best fit among installed and indexed packs, downloaded if needed]
    #[arg(long)]
    pack: Option<PathBuf>,
    #[command(flatten)]
    index: IndexArgs,
    /// Also export the overworld (else the first map) as a Sponge .schem
    #[arg(long)]
    schem: Option<PathBuf>,
    #[command(flatten)]
    download: DownloadArgs,
    /// Same-seed regeneration of the untouched terrain (tools/regen_world.py) for exact underground
    #[arg(long)]
    regen: Option<PathBuf>,
    /// Dimension of the (single) map [default: guessed from the map id, else from BlueMap's default sky colour]
    #[arg(long)]
    dimension: Option<String>,
    /// Continue even if the pack does not fit the site
    #[arg(long)]
    force: bool,
    /// Print per-stage timings and write them as JSON {stage: seconds} here
    #[arg(long)]
    timings: Option<PathBuf>,
    #[command(flatten)]
    opts: OptionArgs,
}

pub fn run(a: Args) -> Result<()> {
    let total = Instant::now();
    let mut t = Timings::default();
    let slug = site_slug(&a.url);
    let data_dir = a.download.data.data_dir()?;
    let cache = a.download.data.mirror_dir(&a.url)?;

    progress!("[1/4] mirroring {} → {}", a.url, cache.display());
    let fetch_opts = a.download.options(&a.url, cache.clone(), a.map.iter().cloned().collect());
    let summaries = t.time("fetch", || bmr_fetch::mirror(&fetch_opts))?;
    let ids: Vec<String> = summaries.iter().map(|s| s.id.clone()).collect();
    let maps = plan::choose(&cache, &ids, a.map.as_deref(), a.dimension.as_deref())?;
    for m in &maps {
        progress!(
            "      map `{}` ({}) → {}, {} hires tiles",
            m.id,
            m.map.settings.name,
            m.dimension,
            m.map.tiles(0).len()
        );
    }
    let first = &maps.first().context("the site has no maps")?.map;

    progress!(
        "[2/4] choosing a pack (texture fingerprint; site runs BlueMap {})",
        first.bluemap_version.as_deref().unwrap_or("?")
    );
    let site_textures = bmr_prbm::parse_texture_names(&first.textures_json()?)?;
    let site_version = first.bluemap_version.as_deref();
    let (pack_path, ranking) = select_pack(a.pack.as_deref(), &a.index, &data_dir, site_version, &site_textures)?;
    for (i, line) in ranking.iter().enumerate() {
        progress!("      {} {line}", if i == 0 { "→" } else { " " });
    }
    let pack = t.time("pack_load", || Pack::load(&pack_path))?;
    let compat = bmr_pack::check(&pack, site_version, &site_textures);
    progress!(
        "      pack {} (Minecraft {}, BlueMap {})",
        pack_path.display(),
        pack.meta.mc_version,
        pack.meta.bluemap_version
    );
    let fits = matches!(compat.verdict(), Verdict::Ok);
    for line in compat.explain() {
        // a misfit's explanation is a warning: shown even with --quiet
        if fits {
            progress!("      {line}");
        } else {
            println!("      {line}");
        }
    }
    match compat.verdict() {
        Verdict::Fail if !a.force => {
            bail!("this pack does not fit the site (see above); use a matching pack or --force")
        }
        Verdict::Fail | Verdict::Warn => println!("      continuing: blocks with unknown textures will be left out"),
        Verdict::Ok => {}
    }

    let name = match &a.map {
        Some(m) => format!("{slug}-{m}"),
        None => slug.clone(),
    };
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from(format!("{name}.zip")));
    let zip = out.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    let world_dir = if zip { cache.join("reconstructed").join(&name).join("world") } else { out.clone() };
    if zip {
        let _ = std::fs::remove_dir_all(world_dir.parent().context("world dir")?);
    }

    for (i, m) in maps.iter().enumerate() {
        let opts = a.opts.options(&m.dimension)?;
        let p = opts.profile;
        let mask = p.mask.map_or(String::new(), |(lo, hi)| format!(", y {lo}..{hi} hidden by the map"));
        progress!("[3/4] reconstructing `{}` ({}, y {}..{}{mask})", m.id, m.dimension, p.min_y, p.max_y);
        let regen = a
            .regen
            .as_ref()
            .map(|r| bmr_world::World::open(r, &m.dimension, Some(pack.registry.clone())))
            .transpose()?;
        let inputs = Inputs {
            map: &m.map,
            lib: &pack.library,
            biome_tints: &pack.biome_tints,
            registry: &pack.registry,
            template: &pack.template,
            style: pack.palette_style(),
            dimension: &m.dimension,
            regen: regen.as_ref(),
            out: &world_dir,
            extend: i > 0,
            opts: &opts,
        };
        let (totals, wt) = reconstruct(&inputs)?;
        t.accumulate(wt);
        let unmatched_pct = 100.0 * totals.unmatched as f64 / totals.cells.max(1) as f64;
        progress!(
            "      {} blocks recognised, {:.2}% unrecognised, {} unseen solid + {} unseen liquid filled, {} chunks{}",
            totals.cells - totals.unmatched,
            unmatched_pct,
            totals.solid,
            totals.liquid,
            totals.chunks,
            totals.overlap_note()
        );
        if unmatched_pct > 1.0 {
            progress!(
                "      note: >1% unrecognised usually means custom models (resource pack/mods) — see the texture sets above"
            );
        }
    }

    progress!("[4/4] writing output");
    if zip {
        let files = t.time("zip", || bmr_world::zip_world(&world_dir, &out))?;
        progress!("      {} ({files} files, {:.1} MB) — extract into your saves/ folder", out.display(), size_mb(&out));
    } else {
        progress!("      world folder {}", out.display());
    }
    if let Some(schem) = &a.schem {
        let m = &maps[0];
        let p = a.opts.options(&m.dimension)?.profile;
        let world = bmr_world::World::open(&world_dir, &m.dimension, Some(pack.registry.clone()))?;
        let extent = world_extent(&world, [p.min_y, p.max_y])?;
        let s = t.time("schem", || bmr_world::export_schem(&world, extent, true, &m.id, schem))?;
        let size: [i32; 3] = std::array::from_fn(|i| s.area.max[i] - s.area.min[i] + 1);
        progress!("      {} ({} {}x{}x{}, {:.1} MB)", schem.display(), m.id, size[0], size[1], size[2], size_mb(schem));
    }
    t.record("total", total.elapsed());
    progress!("done in {:.1?}", total.elapsed());
    if a.timings.is_some() {
        report(&t, a.timings.as_deref())?;
    }
    Ok(())
}

fn size_mb(p: &Path) -> f64 {
    std::fs::metadata(p).map_or(0.0, |m| m.len() as f64 / 1e6)
}
