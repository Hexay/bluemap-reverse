//! Debug one cell: observed faces, matcher candidates, and a diff against the true state's library signature.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use bmr_invert::face::{FaceKey, faces_by_cell, signature, texture_names, world_faces};
use bmr_invert::matcher::candidates;
use bmr_invert::Library;

use crate::{MirrorArgs, WorldArgs};

#[derive(clap::Args)]
pub struct Args {
    #[command(flatten)]
    mirror: MirrorArgs,
    /// Cell x,y,z
    #[arg(allow_hyphen_values = true)]
    cell: String,
    /// Original world: diff the cell against its true state's library signature
    #[arg(long)]
    original: Option<PathBuf>,
    #[arg(long, default_value = "work/cache/debug")]
    library_mirror: PathBuf,
    #[arg(long, default_value = "work/worlds/debug/world")]
    library_world: PathBuf,
    #[command(flatten)]
    world_args: WorldArgs,
}

pub fn run(a: Args) -> Result<()> {
    let v: Vec<i32> = a.cell.split(',').map(str::parse).collect::<Result<_, _>>()?;
    let [x, y, z] = v[..] else { bail!("cell must be x,y,z") };
    let registry = a.world_args.registry()?.context("needs the block registry")?;
    let lib_map = bmr_fetch::LocalMap::open(&a.library_mirror, None)?;
    let lib = Library::build(&lib_map, &a.world_args.open(&a.library_world, &Some(registry.clone()))?, &registry)?;

    let map = a.mirror.open()?;
    let names = texture_names(&bmr_prbm::parse_textures(&map.textures_json()?)?);
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
        let tex = k.texture.trim_start_matches("minecraft:block/");
        let b = if k.on_boundary() { "B" } else { " " };
        println!("  {b} {tex:<28} tint={} {:?}", k.tinted as u8, k.verts);
    }
}
