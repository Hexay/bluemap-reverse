mod check_heights;
mod copy_world;
mod diff_render;
mod explain;
mod fetch;
mod obj;
mod pack;
mod pack_source;
mod paths;
mod probe;
mod pull;
mod reconstruct;
mod reverse;
mod schem;
mod score;
mod seed;
mod structures;
mod ui;
mod window;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Result;
use bmr_fetch::LocalMap;
use bmr_world::{BlockRegistry, World};
use clap::{Parser, Subcommand};

use crate::copy_world::DEFAULT_TEMPLATE;

const EXIT_CODES: &str = "Exit codes: 0 success, 1 error, 2 usage error, 3 `seed` found no seed.
Downloads (site mirrors, packs) go to --data-dir, else $BMR_HOME, else the per-user data dir.";

#[derive(Parser)]
#[command(name = "bmr", version, about = "Reconstruct a Minecraft world from a BlueMap web map", after_help = EXIT_CODES)]
struct Cli {
    /// Print only results, warnings and errors (no progress)
    #[arg(short, long, global = true)]
    quiet: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// One command: mirror a BlueMap site and reconstruct it into a world zip (only maps you own or may reverse).
    Pull(pull::Args),
    /// Build or inspect packs (library + registry + template for one BlueMap/Minecraft version).
    #[command(subcommand)]
    Pack(pack::Cmd),
    /// Mirror a BlueMap site (only maps you own or may reverse).
    Fetch(fetch::Args),
    /// Export a mirrored map's hires tiles as OBJ + MTL + textures.
    Obj(obj::Args),
    /// Cross-check hires top faces against the lowres heightmap.
    #[command(hide = true)]
    CheckHeights(check_heights::Args),
    /// Diff two renders of the same area face by face (original site vs re-rendered reconstruction).
    #[command(hide = true)]
    DiffRender(diff_render::Args),
    /// Score a reconstructed world against the original, block by block.
    Score(score::Args),
    /// Print block states (and biome) at world positions.
    #[command(hide = true)]
    Probe(probe::Args),
    /// Read a world's full chunks and write them back out with our writer (round-trip test).
    #[command(hide = true)]
    CopyWorld(copy_world::Args),
    /// Reconstruct a world from a mirrored map using the debug-world signature library.
    Reverse(reverse::Args),
    /// Debug one cell: observed faces, candidates, diff against the true state's signature.
    Explain(explain::Args),
    /// Export a world area as a Sponge v3 .schem (WorldEdit / FAWE).
    Schem(schem::Args),
    /// Crack the world seed from observed structure starts.
    Seed(seed::Args),
    /// Find structures in a world and write them as `bmr seed` observations.
    Structures(structures::Args),
}

/// Mirror dir + optional map id, shared by commands that read a local mirror.
#[derive(clap::Args)]
struct MirrorArgs {
    /// Mirror dir written by `bmr fetch`, e.g. `work/cache/<fixture>`
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
    /// Dimension to read or write
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

    fn open(
        &self,
        root: &std::path::Path,
        registry: &Option<Arc<bmr_world::BlockRegistry>>,
    ) -> Result<bmr_world::World> {
        bmr_world::World::open(root, &self.dimension, registry.clone())
    }
}

/// Debug-world library sources, template and biome fixture, shared by commands that build the library from
/// local files (`reverse`, `pack build`, `explain`).
#[derive(clap::Args)]
struct LibraryArgs {
    /// Mirror of BlueMap's render of the debug world (signature source)
    #[arg(long, default_value = "work/cache/debug")]
    library_mirror: PathBuf,
    /// The debug world itself (ground-truth states for the library)
    #[arg(long, default_value = "work/worlds/debug/world")]
    library_world: PathBuf,
    /// Empty world whose level.dat/data seed the output
    #[arg(long, default_value = DEFAULT_TEMPLATE)]
    template: PathBuf,
    /// Mirror + world of the `biomes` fixture (tools/gen_biomes.py) for the biome tint table; overworld
    /// biomes stay plains without it
    #[arg(long, default_value = "work/cache/biomes")]
    biome_mirror: PathBuf,
    /// World of the `biomes` fixture (see --biome-mirror)
    #[arg(long, default_value = "work/worlds/biomes/world")]
    biome_world: PathBuf,
}

/// The debug and biome fixtures live in the overworld whatever dimension is being reversed.
const FIXTURE_DIMENSION: &str = "minecraft:overworld";

impl LibraryArgs {
    /// The debug world's mirror and the world itself.
    fn library_sources(&self, registry: &Arc<BlockRegistry>) -> Result<(LocalMap, World)> {
        let map = LocalMap::open(&self.library_mirror, None)?;
        Ok((map, World::open(&self.library_world, FIXTURE_DIMENSION, Some(registry.clone()))?))
    }

    fn library(&self, registry: &Arc<BlockRegistry>) -> Result<bmr_invert::Library> {
        let (map, world) = self.library_sources(registry)?;
        bmr_invert::Library::build(&map, &world, registry)
    }

    /// The biome fixture's mirror and world, or `None` with a warning when either is missing.
    fn biomes_or_warn(&self, registry: &Arc<BlockRegistry>) -> Option<(LocalMap, World)> {
        let opened = LocalMap::open(&self.biome_mirror, None)
            .and_then(|m| Ok((m, World::open(&self.biome_world, FIXTURE_DIMENSION, Some(registry.clone()))?)));
        opened.map_err(|e| eprintln!("warning: no biome tint table ({e}); overworld biomes stay plains")).ok()
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    ui::set_quiet(cli.quiet);
    match run(cli.cmd) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:?}");
            ExitCode::FAILURE
        }
    }
}

fn run(cmd: Cmd) -> Result<ExitCode> {
    match cmd {
        Cmd::Pull(a) => pull::run(a),
        Cmd::Pack(c) => pack::run(c),
        Cmd::Fetch(a) => fetch::run(a),
        Cmd::Obj(a) => obj::run(a),
        Cmd::CheckHeights(a) => check_heights::run(a),
        Cmd::DiffRender(a) => diff_render::run(a),
        Cmd::Score(a) => score::run(a),
        Cmd::Probe(a) => probe::run(a),
        Cmd::CopyWorld(a) => copy_world::run(a),
        Cmd::Reverse(a) => reverse::run(a),
        Cmd::Explain(a) => explain::run(a),
        Cmd::Schem(a) => schem::run(a),
        Cmd::Seed(a) => return seed::run(a),
        Cmd::Structures(a) => structures::run(a),
    }
    .map(|()| ExitCode::SUCCESS)
}
