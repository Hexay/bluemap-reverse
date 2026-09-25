mod check_heights;
mod copy_world;
mod explain;
mod fetch;
mod obj;
mod probe;
mod reverse;
mod score;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bmr", about = "Reconstruct a Minecraft world from a BlueMap web map")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Mirror a BlueMap site (only maps you own or may reverse).
    Fetch(fetch::Args),
    /// Export a mirrored map's hires tiles as OBJ + MTL + textures.
    Obj(obj::Args),
    /// Cross-check hires top faces against the lowres heightmap.
    CheckHeights(check_heights::Args),
    /// Score a reconstructed world against the original, block by block.
    Score(score::Args),
    /// Print block states (and biome) at world positions.
    Probe(probe::Args),
    /// Read a world's full chunks and write them back out with our writer (round-trip test).
    CopyWorld(copy_world::Args),
    /// Reconstruct a world from a mirrored map using the debug-world signature library.
    Reverse(reverse::Args),
    /// Debug one cell: observed faces, candidates, diff against the true state's signature.
    Explain(explain::Args),
}

/// Mirror dir + optional map id, shared by commands that read a local mirror.
#[derive(clap::Args)]
struct MirrorArgs {
    /// Mirror dir written by `bmr fetch`, e.g. work/cache/<fixture>
    #[arg(long)]
    mirror: PathBuf,
    /// Map id (optional when the mirror has one map)
    #[arg(long)]
    map: Option<String>,
}

impl MirrorArgs {
    fn open(&self) -> Result<bmr_fetch::LocalMap> {
        bmr_fetch::LocalMap::open(&self.mirror, self.map.as_deref())
    }
}

/// Dimension + block registry, shared by commands that read Anvil worlds.
#[derive(clap::Args)]
struct WorldArgs {
    #[arg(long, default_value = "minecraft:overworld")]
    dimension: String,
    /// Vanilla blocks.json report (tools/setup.py); needed for 26.3+ palettes
    #[arg(long, default_value = DEFAULT_BLOCKS)]
    blocks: PathBuf,
}

const DEFAULT_BLOCKS: &str = "work/data/reports-26.3/reports/blocks.json";

impl WorldArgs {
    fn registry(&self) -> Result<Option<Arc<bmr_world::BlockRegistry>>> {
        if !self.blocks.exists() {
            eprintln!("note: {} not found; 26.3+ worlds will fail to decode", self.blocks.display());
            return Ok(None);
        }
        Ok(Some(Arc::new(bmr_world::BlockRegistry::load(&self.blocks)?)))
    }

    fn open(&self, root: &std::path::Path, registry: &Option<Arc<bmr_world::BlockRegistry>>) -> Result<bmr_world::World> {
        bmr_world::World::open(root, &self.dimension, registry.clone())
    }
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Fetch(a) => fetch::run(a),
        Cmd::Obj(a) => obj::run(a),
        Cmd::CheckHeights(a) => check_heights::run(a),
        Cmd::Score(a) => score::run(a),
        Cmd::Probe(a) => probe::run(a),
        Cmd::CopyWorld(a) => copy_world::run(a),
        Cmd::Reverse(a) => reverse::run(a),
        Cmd::Explain(a) => explain::run(a),
    }
}
