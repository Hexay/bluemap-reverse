//! Truth of solid cells below the reconstruction's surfaces, by reconstructed biome and depth (how
//! `bmr_fill::material::biome_rock` was measured). Run from the repo root (reads the 26.3 blocks.json):
//! cargo run --release -p bmr-score --example hidden_by_biome -- <original> <reconstructed> <dimension> [max_y] [min_y]

use std::collections::BTreeMap;
use std::path::Path;

use bmr_world::{Dimension, World};

fn bucket(d: i32) -> &'static str {
    match d {
        1 => "d1",
        2..=4 => "d2-4",
        5..=8 => "d5-8",
        9..=16 => "d9-16",
        _ => "d17+",
    }
}

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let reg = Some(std::sync::Arc::new(bmr_world::BlockRegistry::load(Path::new(
        "work/data/reports-26.3/reports/blocks.json",
    ))?));
    let dimension = Dimension::parse(&a[3]);
    let orig = World::open(Path::new(&a[1]), &dimension, reg.clone())?;
    let rec = World::open(Path::new(&a[2]), &dimension, reg)?;
    let max_y: i32 = a.get(4).map_or(10_000, |s| s.parse().unwrap());
    let min_y: i32 = a.get(5).map_or(-10_000, |s| s.parse().unwrap());
    let mut counts: BTreeMap<(String, &str), BTreeMap<String, u64>> = BTreeMap::new();
    for r in orig.regions()? {
        let (o, n) = (orig.read_region(r)?, rec.read_region(r)?);
        for (pos, oc) in &o {
            let Some(nc) = n.get(pos) else { continue };
            for x in 0..16 {
                for z in 0..16 {
                    let mut depth = 0;
                    for ns in nc.sections.iter().rev() {
                        let Some(os) = oc.section(ns.y) else { continue };
                        for ly in (0..16).rev() {
                            let y = ns.y * 16 + ly as i32;
                            let nb = ns.block(x, ly, z);
                            if nb.is_air() || nb.name.contains("lava") || nb.name.contains("water") {
                                depth = 0;
                                continue;
                            }
                            depth += 1;
                            let ob = os.block(x, ly, z);
                            if ob.is_air() || y > max_y || y < min_y {
                                continue;
                            }
                            let biome = ns.biome(x / 4, ly / 4, z / 4).unwrap_or("?").to_owned();
                            *counts.entry((biome, bucket(depth))).or_default().entry(ob.name.clone()).or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    for ((biome, b), m) in &counts {
        let total: u64 = m.values().sum();
        let mut v: Vec<_> = m.iter().collect();
        v.sort_by(|x, y| y.1.cmp(x.1));
        let top: Vec<String> = v
            .iter()
            .take(4)
            .map(|(k, c)| format!("{} {:.0}%", k.trim_start_matches("minecraft:"), 100.0 * **c as f64 / total as f64))
            .collect();
        println!("{biome:<32} {b:<6} n={total:<8} {}", top.join(", "));
    }
    Ok(())
}
