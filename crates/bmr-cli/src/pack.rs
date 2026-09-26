//! `bmr pack build|info|index|list|fetch`: create packs and their index (maintainer), inspect them, and
//! list/download packs from the index (users rarely need to: `pull` downloads the pack it picks).

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use bmr_pack::{Index, Pack, Source};

use crate::WorldArgs;
use crate::copy_world::DEFAULT_TEMPLATE;
use crate::pack_source::{IndexArgs, candidates, download, fetch_index, find_packs};

#[derive(clap::Subcommand)]
pub enum Cmd {
    /// Build a pack from work/cache/debug + work/worlds/debug + the void template
    Build(BuildArgs),
    /// Show a pack's versions and size breakdown
    Info { pack: PathBuf },
    /// Write index.json for the packs in a folder (publish it next to them)
    Index {
        #[arg(default_value = "packs")]
        dir: PathBuf,
    },
    /// Installed and indexed packs
    List(IndexArgs),
    /// Download packs from the index: a Minecraft version, or `all`
    Fetch {
        mc_version: String,
        #[command(flatten)]
        index: IndexArgs,
    },
}

#[derive(clap::Args)]
pub struct BuildArgs {
    /// Output file [default: packs/bmr-mc<version>-bluemap<version>.pack]
    #[arg(short, long)]
    out: Option<PathBuf>,
    #[arg(long, default_value = "26.3")]
    mc_version: String,
    #[arg(long, default_value = "work/cache/debug")]
    library_mirror: PathBuf,
    #[arg(long, default_value = "work/worlds/debug/world")]
    library_world: PathBuf,
    #[arg(long, default_value = DEFAULT_TEMPLATE)]
    template: PathBuf,
    #[command(flatten)]
    world_args: WorldArgs,
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Build(a) => build(a),
        Cmd::Info { pack } => info(&pack),
        Cmd::Index { dir } => index(&dir),
        Cmd::List(a) => list(&a),
        Cmd::Fetch { mc_version, index } => fetch(&mc_version, &index),
    }
}

fn build(a: BuildArgs) -> Result<()> {
    let t = Instant::now();
    let registry = a.world_args.registry()?.context("pack build needs the block registry (tools/setup.py)")?;
    let lib_map = bmr_fetch::LocalMap::open(&a.library_mirror, None)?;
    let lib_world = a.world_args.open(&a.library_world, &Some(registry.clone()))?;
    let pack = Pack::build(&lib_map, &lib_world, registry, &a.template, &a.mc_version)?;
    let out = a.out.unwrap_or_else(|| {
        PathBuf::from("packs").join(format!("bmr-mc{}-bluemap{}.pack", pack.meta.mc_version, pack.meta.bluemap_version))
    });
    let bytes = pack.save(&out)?;
    pack.verify_saved(&out)?;
    println!(
        "{} states, {} registry blocks, {} template files → {} ({:.2} MB) in {:.1?}",
        pack.library.entries.len(),
        pack.registry.len(),
        pack.template.len(),
        out.display(),
        bytes as f64 / 1e6,
        t.elapsed()
    );
    Ok(())
}

fn info(path: &Path) -> Result<()> {
    let t = Instant::now();
    let p = Pack::load(path)?;
    println!(
        "Minecraft {} (data version {}), BlueMap {}, {} states, {} textures, loaded in {:.1?}",
        p.meta.mc_version,
        p.meta.data_version,
        p.meta.bluemap_version,
        p.library.entries.len(),
        p.site_textures.len(),
        t.elapsed()
    );
    for (part, bytes) in Pack::sizes(path)? {
        println!("  {part:<24} {:>9.1} KB", bytes as f64 / 1e3);
    }
    Ok(())
}

fn index(dir: &Path) -> Result<()> {
    let mut packs: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| dir.display().to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "pack"))
        .collect();
    packs.sort();
    let index = Index::build(&packs)?;
    let out = dir.join("index.json");
    index.save(&out)?;
    println!("{} packs, {} texture names → {}", index.packs.len(), index.textures.len(), out.display());
    Ok(())
}

fn list(a: &IndexArgs) -> Result<()> {
    let (all, notes) = candidates(a);
    for c in &all {
        let where_ = match &c.source {
            Source::Installed(p) => p.display().to_string(),
            Source::Indexed(e) => format!("index, {:.2} MB", e.size as f64 / 1e6),
        };
        println!("Minecraft {:<8} BlueMap {:<6} {where_}", c.mc_version, c.bluemap_version);
    }
    notes.iter().for_each(|n| println!("{n}"));
    Ok(())
}

fn fetch(mc_version: &str, a: &IndexArgs) -> Result<()> {
    let index = fetch_index(&a.pack_index)?;
    let wanted: Vec<_> = index.usable().filter(|e| mc_version == "all" || e.mc_version == mc_version).collect();
    if wanted.is_empty() {
        let have: Vec<&str> = index.usable().map(|e| e.mc_version.as_str()).collect();
        bail!("no pack for Minecraft {mc_version} in the index (has: {})", have.join(", "));
    }
    let installed = find_packs();
    for e in wanted {
        if installed.iter().any(|p| p.file_name().is_some_and(|n| n.to_string_lossy() == e.file)) {
            println!("ok      {}", e.file);
            continue;
        }
        println!("fetched {}", download(&a.pack_index, e)?.display());
    }
    Ok(())
}
