//! Per column: highest up-facing hires face (as block y) vs lowres lod-1 height. Agreement validates the
//! PRBM decode + tile→world transform without a human looking at the OBJ.

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use bmr_fetch::grid::Tile;
use bmr_fetch::lowres::LowresImage;

use crate::MirrorArgs;

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    mirror: MirrorArgs,
    /// Mismatches to print
    #[arg(long, default_value_t = 10)]
    show: usize,
}

pub fn run(a: Args) -> Result<()> {
    let map = a.mirror.open()?;
    let textures = bmr_prbm::parse_textures(&map.textures_json()?)?;

    let mut top: HashMap<(i32, i32), i32> = HashMap::new();
    let mut faces = 0;
    let mut per_texture: BTreeMap<&str, usize> = BTreeMap::new();
    for t in map.tiles(0) {
        let tile = bmr_prbm::parse(&map.tile_bytes(0, t)?)?;
        let [ox, oz] = map.hires_origin(t);
        for f in tile.faces() {
            faces += 1;
            *per_texture.entry(textures[f.material as usize].resource_path.as_str()).or_default() += 1;
            if f.normal[1] <= 0 {
                continue;
            }
            let cx = (f.pos.iter().map(|p| p[0]).sum::<f32>() / 3.0).floor() as i32 + ox;
            let cz = (f.pos.iter().map(|p| p[2]).sum::<f32>() / 3.0).floor() as i32 + oz;
            let y = (f.pos.iter().map(|p| p[1]).fold(f32::MIN, f32::max) - 1e-3).floor() as i32;
            let e = top.entry((cx, cz)).or_insert(y);
            *e = (*e).max(y);
        }
    }

    let grid = map.settings.lowres_grid(1);
    let mut images: HashMap<Tile, Option<LowresImage>> = HashMap::new();
    let (mut ok, mut no_lowres, mut bad) = (0, 0, Vec::new());
    let mut columns: Vec<_> = top.into_iter().collect();
    columns.sort();
    for ((x, z), hires_y) in columns {
        let t = grid.tile_of(x, z);
        let img = images.entry(t).or_insert_with(|| {
            map.tile_bytes(1, t).ok().and_then(|b| LowresImage::decode(&b).ok())
        });
        let (mx, mz) = grid.tile_min(t);
        let (px, pz) = ((x - mx) as usize, (z - mz) as usize);
        match img {
            Some(img) if img.color(px, pz)[3] > 0 => {
                let lowres_y = img.block_height(px, pz) as i32;
                if lowres_y == hires_y { ok += 1 } else { bad.push((x, z, hires_y, lowres_y)) }
            }
            _ => no_lowres += 1,
        }
    }

    println!("{faces} faces, {} textures used", per_texture.len());
    let mut by_count: Vec<_> = per_texture.into_iter().collect();
    by_count.sort_by(|a, b| b.1.cmp(&a.1));
    for (name, n) in by_count.iter().take(8) {
        println!("  {n:>6}  {name}");
    }
    let total = ok + bad.len();
    println!(
        "columns: {ok}/{total} heights match ({:.2}%), {} differ, {no_lowres} without lowres colour",
        100.0 * ok as f64 / total.max(1) as f64,
        bad.len()
    );
    for (x, z, h, l) in bad.iter().take(a.show) {
        println!("  x={x} z={z}: hires top y={h}, lowres y={l}");
    }
    Ok(())
}
