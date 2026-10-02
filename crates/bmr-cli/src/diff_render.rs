use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

use anyhow::{Result, ensure};
use bmr_fetch::LocalMap;
use bmr_prbm::Tile;
use bmr_prbm::diff::{RenderDiff, face_cell};

#[derive(clap::Args)]
pub struct Args {
    /// Mirror of the original site
    original: PathBuf,
    /// Mirror of the reconstruction rendered with the same map config (tools/roundtrip.py)
    reconstructed: PathBuf,
    /// Map id (optional when the mirrors hold one map)
    #[arg(long)]
    map: Option<String>,
    /// Drop columns within this many blocks of one the original didn't draw: a relit world that ends there
    /// gets sky light leaking in sideways (up to 15 blocks), lighting caves and the sea near its edge
    #[arg(long, default_value_t = 0)]
    inset: i32,
    /// Textures and cells to list
    #[arg(long, default_value_t = 15)]
    top: usize,
    /// Also write the report as JSON
    #[arg(long)]
    json: Option<PathBuf>,
}

fn texture_names(map: &LocalMap) -> Result<Vec<String>> {
    Ok(bmr_prbm::parse_textures(&map.textures_json()?)?.into_iter().map(|t| t.resource_path).collect())
}

fn hires_tile(map: &LocalMap, present: &BTreeSet<(i32, i32)>, t: (i32, i32)) -> Result<Tile> {
    if present.contains(&t) { bmr_prbm::parse(&map.tile_bytes(0, t)?) } else { Ok(Tile::default()) }
}

fn drawn_columns(map: &LocalMap, tiles: &BTreeSet<(i32, i32)>) -> Result<HashSet<(i32, i32)>> {
    let mut columns = HashSet::new();
    for &t in tiles {
        let origin = map.hires_origin(t);
        columns.extend(bmr_prbm::parse(&map.tile_bytes(0, t)?)?.faces().map(|f| {
            let [x, _, z] = face_cell(&f, origin);
            (x, z)
        }));
    }
    Ok(columns)
}

/// Columns whose whole (2n+1)² neighbourhood is in `columns`.
fn inset(columns: HashSet<(i32, i32)>, n: i32) -> HashSet<(i32, i32)> {
    let inside = |&(x, z): &(i32, i32)| (-n..=n).all(|dx| (-n..=n).all(|dz| columns.contains(&(x + dx, z + dz))));
    columns.iter().copied().filter(inside).collect()
}

pub fn run(a: Args) -> Result<()> {
    let original = LocalMap::open(&a.original, a.map.as_deref())?;
    let reconstructed = LocalMap::open(&a.reconstructed, a.map.as_deref())?;
    ensure!(
        original.settings.hires_grid() == reconstructed.settings.hires_grid(),
        "the mirrors use different hires tile grids"
    );
    let (original_names, reconstructed_names) = (texture_names(&original)?, texture_names(&reconstructed)?);
    let original_tiles: BTreeSet<_> = original.tiles(0).into_iter().collect();
    let reconstructed_tiles: BTreeSet<_> = reconstructed.tiles(0).into_iter().collect();

    let columns = inset(drawn_columns(&original, &original_tiles)?, a.inset);
    let unrendered = |x, z| !columns.contains(&(x, z));
    let mut diff = RenderDiff::default();
    for &t in &original_tiles {
        diff.add_tile(
            &hires_tile(&original, &original_tiles, t)?,
            &original_names,
            &hires_tile(&reconstructed, &reconstructed_tiles, t)?,
            &reconstructed_names,
            original.hires_origin(t),
            unrendered,
        );
    }
    let report = diff.finish(a.top);
    print!("{report}");
    if let Some(path) = a.json {
        std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    }
    Ok(())
}
