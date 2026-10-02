use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use bmr_world::World;
use rustc_hash::FxHashMap;

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
    /// Map id in --mirror (optional when the mirror has one map)
    #[arg(long, requires = "mirror")]
    map: Option<String>,
    /// Only score columns in x0,z0,x1,z1 (inclusive)
    // see schem.rs: num_args breaks "--rect=a,b,c,d"
    #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
    rect: Vec<i32>,
    /// Confusions to list per section of the report
    #[arg(long, default_value_t = 15)]
    top: usize,
    /// Print positions of one confusion: `"<original>-><reconstructed>"` as printed in the report
    #[arg(long)]
    sample: Option<String>,
    /// Also write the report as JSON
    #[arg(long)]
    json: Option<PathBuf>,
    /// Pack whose library defines look-alike states: adds the look-alike-tolerant rendered metric and
    /// drops look-alikes from the rendered confusions
    #[arg(long)]
    pack: Option<PathBuf>,
}

/// State label → look-alike group id.
fn lookalike_groups(pack: &Path) -> Result<FxHashMap<String, u32>> {
    let lib = bmr_pack::Pack::load(pack)?.library;
    let mut groups = FxHashMap::default();
    for (id, g) in lib.lookalikes().into_iter().enumerate() {
        groups.extend(g.into_iter().map(|i| (lib.entries[i].state.to_string(), id as u32)));
    }
    Ok(groups)
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
    anyhow::ensure!(a.rect.is_empty() || a.rect.len() == 4, "--rect takes x0,z0,x1,z1");
    let rect = match a.rect[..] {
        [x0, z0, x1, z1] => Some([x0.min(x1), z0.min(z1), x0.max(x1), z0.max(z1)]),
        _ => None,
    };
    let filter = move |x: i32, z: i32| {
        let in_rect = rect.is_none_or(|r| r[0] <= x && x <= r[2] && r[1] <= z && z <= r[3]);
        in_rect && tiles.as_ref().is_none_or(|(tiles, grid)| tiles.contains(&grid.tile_of(x, z)))
    };

    let t = Instant::now();
    let sample = a.sample.as_deref().and_then(|s| s.split_once("->")).map(|(o, r)| (o.trim(), r.trim()));
    let lookalikes = a.pack.as_deref().map(lookalike_groups).transpose()?;
    let scope = bmr_score::Scope { columns: &filter, rendered: rendered.as_ref(), sample, lookalikes: lookalikes.as_ref() };
    let report = bmr_score::score(&original, &reconstructed, &scope, a.top)?;
    print!("{report}");
    println!("scored in {:.1?}", t.elapsed());
    if let Some(path) = a.json {
        std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    }
    Ok(())
}
