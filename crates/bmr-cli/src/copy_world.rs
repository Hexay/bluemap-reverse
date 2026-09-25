//! Identity pass through the reader and writer: proves our chunk encoding round-trips.

use std::path::PathBuf;

use anyhow::{Context, Result};
use bmr_world::WorldWriter;

use crate::WorldArgs;

pub const DEFAULT_TEMPLATE: &str = "work/worlds/template-void/world";

#[derive(clap::Args)]
pub struct Args {
    source: PathBuf,
    out: PathBuf,
    /// World whose level.dat/data seed the output
    #[arg(long, default_value = DEFAULT_TEMPLATE)]
    template: PathBuf,
    #[command(flatten)]
    world_args: WorldArgs,
}

pub fn run(a: Args) -> Result<()> {
    let registry = a.world_args.registry()?.context("copy-world needs the block registry")?;
    let source = a.world_args.open(&a.source, &Some(registry.clone()))?;
    let mut chunks = Vec::new();
    for r in source.regions()? {
        chunks.extend(source.read_region(r)?.into_values().filter(|c| c.is_full()));
    }
    let writer = WorldWriter::create(&a.out, &a.template, &a.world_args.dimension, registry)?;
    let n = chunks.len();
    let regions = writer.write_chunks(chunks)?;
    println!("{n} chunks in {regions} regions → {}", a.out.display());
    Ok(())
}
