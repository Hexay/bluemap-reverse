use std::path::PathBuf;

use anyhow::Result;

use crate::WorldArgs;

#[derive(clap::Args)]
pub struct Args {
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
        let (cx, cz) = (x.div_euclid(16), z.div_euclid(16));
        let chunks = world.read_region((cx.div_euclid(32), cz.div_euclid(32)))?;
        let (lx, lz) = (x.rem_euclid(16) as usize, z.rem_euclid(16) as usize);
        let Some(chunk) = chunks.get(&(cx, cz)) else {
            println!("{x},{y},{z}: chunk not generated");
            continue;
        };
        let state = chunk.block(lx, y, lz).map_or("minecraft:air (no section)".to_owned(), |b| b.to_string());
        let biome = chunk
            .section(y.div_euclid(16))
            .and_then(|s| s.biome(lx / 4, y.rem_euclid(16) as usize / 4, lz / 4))
            .unwrap_or("-");
        println!("{x},{y},{z}: {state}  biome={biome}");
    }
    Ok(())
}
