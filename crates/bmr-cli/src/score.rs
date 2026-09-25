use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use bmr_world::World;

use crate::WorldArgs;

#[derive(clap::Args)]
pub struct Args {
    /// Original world root
    original: PathBuf,
    /// Reconstructed world root [default: an all-air world, i.e. the "nothing recovered" baseline]
    reconstructed: Option<PathBuf>,
    #[command(flatten)]
    world_args: WorldArgs,
    /// Only score columns inside hires tiles of this mirror (the rendered area)
    #[arg(long)]
    mirror: Option<PathBuf>,
    #[arg(long, requires = "mirror")]
    map: Option<String>,
    /// Only score columns in x0,z0,x1,z1 (inclusive)
    #[arg(long, value_delimiter = ',', num_args = 4)]
    rect: Vec<i32>,
    #[arg(long, default_value_t = 15)]
    top: usize,
    /// Also write the report as JSON
    #[arg(long)]
    json: Option<PathBuf>,
}

pub fn run(a: Args) -> Result<()> {
    let registry = a.world_args.registry()?;
    let original = a.world_args.open(&a.original, &registry)?;
    let reconstructed = match &a.reconstructed {
        Some(p) => a.world_args.open(p, &registry)?,
        None => World::empty(PathBuf::from("<no reconstruction>")),
    };

    let map = a.mirror.as_ref().map(|m| bmr_fetch::LocalMap::open(m, a.map.as_deref())).transpose()?;
    let tiles: Option<(HashSet<(i32, i32)>, bmr_fetch::grid::Grid)> =
        map.as_ref().map(|m| (m.tiles(0).into_iter().collect(), m.settings.hires_grid()));
    let rendered = map.as_ref().map(bmr_invert::rendered_cells).transpose()?;
    let rect = a.rect.clone();
    let filter = move |x: i32, z: i32| {
        let in_rect = rect.is_empty() || (rect[0] <= x && x <= rect[2] && rect[1] <= z && z <= rect[3]);
        in_rect && tiles.as_ref().is_none_or(|(tiles, grid)| tiles.contains(&grid.tile_of(x, z)))
    };

    let t = Instant::now();
    let scope = bmr_score::Scope { columns: &filter, rendered: rendered.as_ref() };
    let report = bmr_score::score(&original, &reconstructed, &scope, a.top)?;
    print!("{report}");
    println!("scored in {:.1?}", t.elapsed());
    if let Some(path) = a.json {
        std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    }
    Ok(())
}
