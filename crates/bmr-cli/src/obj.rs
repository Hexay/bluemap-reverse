use std::path::PathBuf;

use anyhow::{Context, Result};
use bmr_fetch::grid::tile_path;
use bmr_prbm::obj::{ObjOptions, PlacedTile, write_obj};

use crate::MirrorArgs;

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    mirror: MirrorArgs,
    /// Output dir [default: work/obj/MAP]
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// Bake AO and light into vertex colours (closer to the web view)
    #[arg(long)]
    shade: bool,
}

pub fn run(a: Args) -> Result<()> {
    let map = a.mirror.open()?;
    let textures = bmr_prbm::parse_textures(&map.textures_json()?)?;
    let tiles = map
        .tiles(0)
        .into_iter()
        .map(|t| {
            let tile = bmr_prbm::parse(&map.tile_bytes(0, t)?).with_context(|| tile_path(t))?;
            Ok((t, tile))
        })
        .collect::<Result<Vec<_>>>()?;
    let placed: Vec<PlacedTile> = tiles
        .iter()
        .map(|(t, tile)| PlacedTile { name: format!("tile_{}_{}", t.0, t.1), tile, origin: map.hires_origin(*t) })
        .collect();

    let out = a.out.unwrap_or_else(|| PathBuf::from("work/obj").join(&map.id));
    let opts = ObjOptions { shade: a.shade, ambient: map.settings.ambient_light };
    let stats = write_obj(&out, &map.id, &placed, &textures, opts)?;
    println!(
        "{} tiles, {} faces, {} materials → {}",
        placed.len(),
        stats.faces,
        stats.materials,
        out.join(format!("{}.obj", map.id)).display()
    );
    Ok(())
}
