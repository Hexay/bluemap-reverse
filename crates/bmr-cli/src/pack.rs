//! `bmr pack build|info`: create a pack from the local debug world (maintainer), or describe one.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use bmr_pack::Pack;

use crate::WorldArgs;
use crate::copy_world::DEFAULT_TEMPLATE;

#[derive(clap::Subcommand)]
pub enum Cmd {
    /// Build a pack from work/cache/debug + work/worlds/debug + the void template
    Build(BuildArgs),
    /// Show a pack's versions and size
    Info { pack: PathBuf },
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
    println!(
        "{} states, {} registry blocks, {} template files → {} ({:.1} MB) in {:.1?}",
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
    Ok(())
}

/// Candidate pack files: `packs/` in the working directory and next to the executable.
pub fn find_packs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("packs")];
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        dirs.push(exe_dir.join("packs"));
    }
    let mut out: Vec<PathBuf> = dirs
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "pack"))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The pack to use: explicit, else the one named for the site's BlueMap version, else the only one.
pub fn choose_pack(explicit: Option<&Path>, site_version: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p.to_path_buf());
    }
    let packs = find_packs();
    if let Some(v) = site_version {
        if let Some(p) = packs.iter().find(|p| p.to_string_lossy().contains(&format!("bluemap{v}."))) {
            return Ok(p.clone());
        }
    }
    match packs.as_slice() {
        [only] => Ok(only.clone()),
        [] => bail!("no .pack file found in ./packs or next to bmr; pass --pack <file>"),
        many => bail!("several packs, none named for BlueMap {:?}; pass --pack (have: {many:?})", site_version),
    }
}
