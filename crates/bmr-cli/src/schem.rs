//! `bmr schem`: export a world area (a reconstruction or any world) as a Sponge v3 `.schem`.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Result, ensure};
use bmr_fetch::BlockBox;
use bmr_world::coords::{chunk_of, local, region_of_chunk};
use bmr_world::{Area, World, export_schem, read_schem};

use crate::WorldArgs;

#[derive(clap::Args)]
pub struct Args {
    /// World root
    world: PathBuf,
    /// Output .schem file
    #[arg(short, long)]
    out: PathBuf,
    /// Columns x0,z0,x1,z1 (inclusive) [default: every region file of the world]
    #[arg(long, allow_hyphen_values = true)]
    area: Option<BlockBox>,
    /// Height range y0,y1 (inclusive)
    #[arg(long, value_delimiter = ',', allow_hyphen_values = true, default_values_t = [-64, 319])]
    y: Vec<i32>,
    /// Keep the full box instead of shrinking it to the non-air blocks inside
    #[arg(long)]
    no_trim: bool,
    /// Schematic name stored in its metadata [default: output file stem]
    #[arg(long)]
    name: Option<String>,
    /// Read the file back and compare every cell with the world
    #[arg(long)]
    verify: bool,
    #[command(flatten)]
    world_args: WorldArgs,
}

pub fn run(a: Args) -> Result<()> {
    ensure!(a.y.len() == 2, "--y takes y0,y1");
    let registry = a.world_args.registry()?;
    let world = a.world_args.open(&a.world, &registry)?;
    let area = match a.area {
        Some(b) => with_heights(b, [a.y[0], a.y[1]]),
        None => world_extent(&world, [a.y[0], a.y[1]])?,
    };
    let name =
        a.name.clone().unwrap_or_else(|| a.out.file_stem().map_or("bmr".into(), |s| s.to_string_lossy().into_owned()));
    let t = Instant::now();
    let s = export_schem(&world, area, !a.no_trim, &name, &a.out)?;
    let size: [i32; 3] = std::array::from_fn(|i| s.area.max[i] - s.area.min[i] + 1);
    println!(
        "{} x {} x {} (from {:?}) · {} states · {} non-air · {} block entities · {} KB → {} in {:.1?}",
        size[0],
        size[1],
        size[2],
        s.area.min,
        s.palette,
        s.non_air,
        s.block_entities,
        std::fs::metadata(&a.out)?.len() / 1024,
        a.out.display(),
        t.elapsed()
    );
    if a.verify {
        verify(&world, &s.area, &a.out)?;
    }
    Ok(())
}

pub fn with_heights(b: BlockBox, y: [i32; 2]) -> Area {
    Area { min: [b.min[0], y[0], b.min[1]], max: [b.max[0], y[1], b.max[1]] }
}

/// Box covering every region file between heights `y` (trimming then shrinks it to the actual blocks).
pub fn world_extent(world: &World, y: [i32; 2]) -> Result<Area> {
    let regions = world.regions()?;
    ensure!(!regions.is_empty(), "world has no region files");
    let (rx0, rx1) = (regions.iter().map(|r| r.0).min().unwrap(), regions.iter().map(|r| r.0).max().unwrap());
    let (rz0, rz1) = (regions.iter().map(|r| r.1).min().unwrap(), regions.iter().map(|r| r.1).max().unwrap());
    Ok(Area { min: [rx0 * 512, y[0], rz0 * 512], max: [rx1 * 512 + 511, y[1], rz1 * 512 + 511] })
}

/// Every schematic cell equals the world's block (air where the world has no section).
fn verify(world: &World, area: &Area, path: &std::path::Path) -> Result<()> {
    let (size, cells) = read_schem(path)?;
    let [w, _, l] = size;
    let mut chunks = rustc_hash::FxHashMap::default();
    let mut mismatches = 0usize;
    for (i, got) in cells.iter().enumerate() {
        let (x, z, y) =
            (area.min[0] + (i % w) as i32, area.min[2] + (i / w % l) as i32, area.min[1] + (i / (w * l)) as i32);
        let cp = chunk_of(x, z);
        if !chunks.contains_key(&cp) {
            chunks.extend(world.read_region(region_of_chunk(cp.0, cp.1))?);
        }
        let want = chunks
            .get(&cp)
            .and_then(|c| c.block(local(x), y, local(z)))
            .filter(|b| !b.is_air())
            .map_or_else(|| "minecraft:air".to_owned(), ToString::to_string);
        if *got != want {
            mismatches += 1;
        }
    }
    ensure!(mismatches == 0, "verify: {mismatches} of {} cells differ", cells.len());
    println!("verify: all {} cells match the world", cells.len());
    Ok(())
}
