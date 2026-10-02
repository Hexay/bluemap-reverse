//! Debug one cell: observed faces, matcher candidates, and a diff against the true state's library signature.
//! Or one column: what the full inversion matched down it and the evidence the fill classifies its gaps by.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use bmr_fetch::LocalMap;
use bmr_invert::Library;
use bmr_invert::face::{FaceKey, faces_by_cell, signature, texture_ids, world_faces};
use bmr_invert::matcher::candidates;

use crate::{LibraryArgs, MirrorArgs, WorldArgs};

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    mirror: MirrorArgs,
    /// Cell x,y,z, or column x,z (matched blocks and fill evidence down the column)
    #[arg(allow_hyphen_values = true)]
    cell: String,
    /// Original world: diff the cell against its true state's library signature
    #[arg(long)]
    original: Option<PathBuf>,
    #[command(flatten)]
    library: LibraryArgs,
    #[command(flatten)]
    world_args: WorldArgs,
}

pub fn run(a: Args) -> Result<()> {
    let v: Vec<i32> = a.cell.split(',').map(str::parse).collect::<Result<_, _>>()?;
    let registry = a.world_args.registry()?.context("needs the block registry")?;
    let lib = a.library.library(&registry)?;
    let map = a.mirror.open()?;
    let [x, y, z] = match v[..] {
        [x, z] => return column(&map, &lib, x, z),
        [x, y, z] => [x, y, z],
        _ => bail!("expected x,y,z (cell) or x,z (column)"),
    };

    let names = texture_ids(&bmr_prbm::parse_texture_names(&map.textures_json()?)?);
    let t = map.settings.hires_grid().tile_of(x, z);
    let tile = bmr_prbm::parse(&map.tile_bytes(0, t)?)?;
    let cells = faces_by_cell(&world_faces(&tile, map.hires_origin(t), &names));
    let observed = signature(cells.get(&(x, y, z)).map(|c| c.keys.clone()).unwrap_or_default());
    println!("observed ({} quads):", observed.len());
    print_keys(&observed);

    match candidates(&lib, &observed) {
        Some(c) => {
            println!("candidates ({:?}):", c.how);
            for id in c.ids.iter().take(10) {
                println!("  {}", lib.entries[*id].state);
            }
        }
        None => println!("no candidates"),
    }

    if let Some(orig) = a.original {
        let world = a.world_args.open(&orig, &Some(registry))?;
        let chunks = world.read_region((x.div_euclid(512), z.div_euclid(512)))?;
        let truth = chunks
            .get(&(x.div_euclid(16), z.div_euclid(16)))
            .and_then(|c| c.block(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize))
            .context("cell not in original")?;
        println!("true state: {truth}");
        let Some(e) = lib.entries.iter().find(|e| &e.state == truth) else {
            println!("  (not in library)");
            return Ok(());
        };
        println!("library signature minus observed:");
        print_keys(&diff(&e.sig, &observed));
        println!("observed minus library signature:");
        print_keys(&diff(&observed, &e.sig));
    }
    Ok(())
}

/// Inverts the column's tile plus its neighbours (overhang and evidence need them), top-down.
fn column(map: &LocalMap, lib: &Library, x: i32, z: i32) -> Result<()> {
    let (tx, tz) = map.settings.hires_grid().tile_of(x, z);
    let present = map.tiles(0);
    let tiles: Vec<_> =
        (-1..=1).flat_map(|dx| (-1..=1).map(move |dz| (tx + dx, tz + dz))).filter(|t| present.contains(t)).collect();
    let inv = bmr_invert::reverse(map, lib, &tiles, &bmr_invert::map_textures(map)?)?;
    let ev = &inv.evidence;
    let cells = inv.blocks.keys().chain(inv.liquids.keys()).chain(&ev.solid).chain(&ev.open).chain(ev.liquid.keys());
    let mut ys: Vec<i32> = cells.filter(|c| (c.0, c.2) == (x, z)).map(|c| c.1).collect();
    ys.sort_unstable_by(|a, b| b.cmp(a));
    ys.dedup();
    for y in ys {
        let c = (x, y, z);
        let block = inv.blocks.get(&c).map(|&e| lib.entries[e].state.to_string());
        let liquid = inv.liquids.get(&c).map(|l| format!("{l:?}"));
        let mut evidence: Vec<String> = Vec::new();
        evidence.extend(ev.solid.contains(&c).then(|| "solid".into()));
        evidence.extend(ev.open.contains(&c).then(|| "open".into()));
        evidence.extend(ev.liquid.get(&c).map(|l| format!("liquid {l:?}")));
        let seen = block.or(liquid).unwrap_or_else(|| "-".into());
        println!("{y:>5}  {seen:<50} {}", evidence.join(", "));
    }
    Ok(())
}

fn diff(a: &[FaceKey], b: &[FaceKey]) -> Vec<FaceKey> {
    let mut rest = b.to_vec();
    a.iter()
        .filter(|k| match rest.iter().position(|r| r == *k) {
            Some(i) => {
                rest.remove(i);
                false
            }
            None => true,
        })
        .cloned()
        .collect()
}

fn print_keys(keys: &[FaceKey]) {
    for k in keys {
        let name = k.texture.name();
        let tex = name.trim_start_matches("minecraft:block/");
        let b = if k.on_boundary() { "B" } else { " " };
        println!("  {b} {tex:<28} tint={} {:?}", k.tinted as u8, k.verts);
    }
}
