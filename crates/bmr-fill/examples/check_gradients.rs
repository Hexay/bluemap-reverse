//! `Gradients` against an original world: share of bedrock/deepslate/stone(/netherrack) cells in the
//! gradient bands predicted exactly, vs the profile's majority by height. Run from the repo root:
//! cargo run --release -p bmr-fill --example check_gradients -- <world> <dimension> <seed>

use std::path::Path;
use std::sync::Arc;

use bmr_fill::Profile;
use bmr_fill::gradient::Gradients;
use bmr_world::{BlockRegistry, Dimension, World};

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let reg = Arc::new(BlockRegistry::load(Path::new("work/data/reports-26.3/reports/blocks.json"))?);
    let dimension = Dimension::parse(&a[2]);
    let world = World::open(Path::new(&a[1]), &dimension, Some(reg))?;
    let profile = Profile::for_dimension(dimension.clone());
    let g = Gradients::new(a[3].parse()?, &profile);
    let bands: Vec<i32> = match dimension {
        Dimension::Nether => (0..=5).chain(122..=127).collect(),
        _ => (-64..=-59).chain(0..=8).collect(),
    };
    let (mut n, mut seeded, mut majority) = (0u64, 0u64, 0u64);
    for r in world.regions()? {
        for c in world.read_region(r)?.values() {
            for &y in &bands {
                let Some(s) = c.section(y.div_euclid(16)) else { continue };
                for lx in 0..16 {
                    for lz in 0..16 {
                        let truth = &s.block(lx, y.rem_euclid(16) as usize, lz).name;
                        if !profile.default_blocks().contains(&truth.as_str()) {
                            continue;
                        }
                        let (x, z) = (c.x * 16 + lx as i32, c.z * 16 + lz as i32);
                        n += 1;
                        seeded += u64::from(g.block(x, y, z) == truth);
                        majority += u64::from(profile.default_block(y) == truth);
                    }
                }
            }
        }
    }
    let pct = |k: u64| 100.0 * k as f64 / n.max(1) as f64;
    println!("cells {n}: seeded {:.2}%, majority {:.2}%", pct(seeded), pct(majority));
    Ok(())
}
