mod check_heights;
mod fetch;
mod obj;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

const DEFAULT_MIRROR: &str = "work/cache/127.0.0.1_8100";

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
}

/// Mirror dir + optional map id, shared by commands that read a local mirror.
#[derive(clap::Args)]
struct MirrorArgs {
    #[arg(long, default_value = DEFAULT_MIRROR)]
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

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Fetch(a) => fetch::run(a),
        Cmd::Obj(a) => obj::run(a),
        Cmd::CheckHeights(a) => check_heights::run(a),
    }
}
