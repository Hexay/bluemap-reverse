//! `bmr pack build|info|index|list|fetch`: create packs and their index (maintainer), inspect them, and
//! list/download packs from the index (users rarely need to: `pull` downloads the pack it picks).

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use bmr_pack::{Index, Pack, Source};

use crate::{LibraryArgs, WorldArgs};
use crate::pack_source::{IndexArgs, candidates, download, fetch_index, find_packs};
use crate::paths::DataArgs;

#[derive(clap::Subcommand)]
pub enum Cmd {
    /// Build a pack from work/cache/debug + work/worlds/debug + the void template
    Build(Box<BuildArgs>),
    /// Show a pack's versions and size breakdown
    Info {
        /// Pack file
        pack: PathBuf,
    },
    /// Write the groups of states that render identically (indistinguishable from tiles) as JSON
    Lookalikes {
        /// Pack file
        pack: PathBuf,
        /// Output .json file
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Write index.json for the packs in a folder (publish it next to them)
    Index {
        /// Folder of .pack files
        #[arg(default_value = "packs")]
        dir: PathBuf,
    },
    /// Installed and indexed packs
    List {
        #[command(flatten)]
        index: IndexArgs,
        #[command(flatten)]
        data: DataArgs,
    },
    /// Download packs from the index into the data dir
    Fetch {
        /// Minecraft version, or `all`
        mc_version: String,
        #[command(flatten)]
        index: IndexArgs,
        #[command(flatten)]
        data: DataArgs,
    },
}

#[derive(clap::Args)]
pub struct BuildArgs {
    /// Output file [default: packs/bmr-mcMC-bluemapBLUEMAP.pack]
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// Minecraft version the debug world was generated with
    #[arg(long, default_value = "26.3")]
    mc: String,
    #[command(flatten)]
    library: LibraryArgs,
    #[command(flatten)]
    world_args: WorldArgs,
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Build(a) => build(*a),
        Cmd::Info { pack } => info(&pack),
        Cmd::Lookalikes { pack, out } => lookalikes(&pack, &out),
        Cmd::Index { dir } => index(&dir),
        Cmd::List { index, data } => list(&index, &data.data_dir()?),
        Cmd::Fetch { mc_version, index, data } => fetch(&mc_version, &index, &data.data_dir()?),
    }
}

fn build(a: BuildArgs) -> Result<()> {
    let t = Instant::now();
    let registry = a.world_args.registry()?.context("pack build needs the block registry (tools/setup.py)")?;
    let (lib_map, lib_world) = a.library.library_sources(&registry)?;
    let biomes = a.library.biomes_or_warn(&registry);
    let pack = Pack::build(&lib_map, &lib_world, registry, &a.library.template, &a.mc,biomes.as_ref().map(|(m, w)| (m, w)))?;
    let out = a.out.unwrap_or_else(|| {
        PathBuf::from("packs").join(format!("bmr-mc{}-bluemap{}.pack", pack.meta.mc_version, pack.meta.bluemap_version))
    });
    let bytes = pack.save(&out)?;
    pack.verify_saved(&out)?;
    println!(
        "{} states, {} registry blocks, {} biomes, {} template files → {} ({:.2} MB) in {:.1?}",
        pack.library.entries.len(),
        pack.registry.len(),
        pack.biome_tints.len(),
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

fn lookalikes(path: &Path, out: &Path) -> Result<()> {
    let lib = Pack::load(path)?.library;
    let groups: Vec<Vec<String>> = lib
        .lookalikes()
        .into_iter()
        .map(|g| g.into_iter().map(|i| lib.entries[i].state.to_string()).collect())
        .collect();
    let states: usize = groups.iter().map(Vec::len).sum();
    std::fs::write(out, serde_json::to_vec(&groups)?)?;
    println!("{} of {} states in {} look-alike groups → {}", states, lib.entries.len(), groups.len(), out.display());
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

fn list(a: &IndexArgs, data_dir: &Path) -> Result<()> {
    let (all, notes) = candidates(a, data_dir);
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

fn fetch(mc_version: &str, a: &IndexArgs, data_dir: &Path) -> Result<()> {
    let index = fetch_index(&a.pack_index)?;
    let wanted: Vec<_> = index.usable().filter(|e| mc_version == "all" || e.mc_version == mc_version).collect();
    if wanted.is_empty() {
        let have: Vec<&str> = index.usable().map(|e| e.mc_version.as_str()).collect();
        bail!("no pack for Minecraft {mc_version} in the index (has: {})", have.join(", "));
    }
    let installed = find_packs(data_dir);
    for e in wanted {
        if installed.iter().any(|p| p.file_name().is_some_and(|n| n.to_string_lossy() == e.file)) {
            println!("ok      {}", e.file);
            continue;
        }
        println!("fetched {}", download(&a.pack_index, e, data_dir)?.display());
    }
    Ok(())
}
