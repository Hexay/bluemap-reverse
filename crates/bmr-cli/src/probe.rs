use std::path::PathBuf;

use anyhow::Result;
use bmr_world::coords::{chunk_of, local, region_of_chunk, section_of};

use crate::WorldArgs;

#[derive(clap::Args)]
pub struct Args {
    /// World root
    world: PathBuf,
    /// One or more x,y,z positions
    #[arg(required = true, allow_hyphen_values = true)]
    positions: Vec<String>,
    #[command(flatten)]
    world_args: WorldArgs,
}

pub fn run(a: Args) -> Result<()> {
    let registry = a.world_args.registry()?;
    let world = a.world_args.open(&a.world, &registry)?;
    for p in &a.positions {
        let v: Vec<i32> = p.split(',').map(str::parse).collect::<Result<_, _>>()?;
        let [x, y, z] = v[..] else { anyhow::bail!("position must be x,y,z: {p}") };
        let (cx, cz) = chunk_of(x, z);
        let chunks = world.read_region(region_of_chunk(cx, cz))?;
        let (lx, lz) = (local(x), local(z));
        let Some(chunk) = chunks.get(&(cx, cz)) else {
            println!("{x},{y},{z}: chunk not generated");
            continue;
        };
        let state = chunk.block(lx, y, lz).map_or("minecraft:air (no section)".to_owned(), |b| b.to_string());
        let biome = chunk.section(section_of(y)).and_then(|s| s.biome(lx / 4, local(y) / 4, lz / 4)).unwrap_or("-");
        println!("{x},{y},{z}: {state}  biome={biome}");
    }
    Ok(())
}
