//! Truth of the nether's masked roof band (y 90..127) by what the reconstruction has just under it: rock at
//! y 89, else the horizontal distance to the nearest column that has (how `columns.rs` roof rules were
//! measured). Prints % non-air per y and distance bucket. Run from the repo root:
//! cargo run --release -p bmr-score --example roof_band -- <original> <reconstructed> [lo] [hi] [csv]

use std::collections::VecDeque;
use std::path::Path;

use bmr_world::World;
use rustc_hash::FxHashMap;

const BUCKETS: [(i32, i32); 9] = [(0, 0), (1, 1), (2, 2), (3, 4), (5, 8), (9, 16), (17, 32), (33, 64), (65, 999)];

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let lo: i32 = a.get(3).map_or(90, |s| s.parse().unwrap());
    let hi: i32 = a.get(4).map_or(127, |s| s.parse().unwrap());
    let reg = Some(std::sync::Arc::new(bmr_world::BlockRegistry::load(Path::new(
        "work/data/reports-26.3/reports/blocks.json",
    ))?));
    let csv = a.get(5).is_some_and(|s| s == "csv");
    let dim = bmr_world::Dimension::Nether;
    let orig = World::open(Path::new(&a[1]), &dim, reg.clone())?;
    let rec = World::open(Path::new(&a[2]), &dim, reg)?;
    let solid = |b: Option<&bmr_world::BlockState>| b.is_some_and(|b| !b.is_air() && !b.name.contains("lava"));
    // per column: rock at lo-1 in the reconstruction, truth bits for lo..=hi
    let mut cols: FxHashMap<(i32, i32), (bool, u64)> = FxHashMap::default();
    for r in orig.regions()? {
        let (o, n) = (orig.read_region(r)?, rec.read_region(r)?);
        for (pos, oc) in &o {
            let Some(nc) = n.get(pos) else { continue };
            for x in 0..16 {
                for z in 0..16 {
                    if (0..lo).all(|y| nc.block(x, y, z).is_none_or(|b| b.is_air())) {
                        continue;
                    }
                    let truth = (lo..=hi)
                        .filter(|&y| oc.block(x, y, z).is_some_and(|b| !b.is_air()))
                        .fold(0u64, |m, y| m | 1 << (y - lo));
                    let key = (pos.0 * 16 + x as i32, pos.1 * 16 + z as i32);
                    let floor = (0..lo - 1).rev().find(|&y| solid(nc.block(x, y, z))).unwrap_or(-1);
                    if csv {
                        let rock = solid(nc.block(x, lo - 1, z)) as u8;
                        let biome = nc.section(4).and_then(|s| s.biome(x / 4, 0, z / 4)).unwrap_or("?");
                        println!("{},{},{rock},{truth},{floor},{biome}", key.0, key.1);
                    }
                    cols.insert(key, (solid(nc.block(x, lo - 1, z)), truth));
                }
            }
        }
    }
    let mut dist: FxHashMap<(i32, i32), i32> = FxHashMap::default();
    let mut q: VecDeque<(i32, i32)> = cols.iter().filter(|(_, v)| v.0).map(|(&c, _)| c).collect();
    q.iter().for_each(|&c| _ = dist.insert(c, 0));
    while let Some((x, z)) = q.pop_front() {
        let d = dist[&(x, z)];
        for n in [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)] {
            if cols.contains_key(&n) && !dist.contains_key(&n) {
                dist.insert(n, d + 1);
                q.push_back(n);
            }
        }
    }
    let nb = BUCKETS.len();
    let rows = (hi - lo + 1) as usize;
    let mut rock = vec![vec![0u64; nb]; rows];
    let mut n = vec![0u64; nb];
    for (c, &(_, truth)) in &cols {
        let d = dist.get(c).copied().unwrap_or(999);
        let b = BUCKETS.iter().position(|&(l, h)| (l..=h).contains(&d)).unwrap_or(nb - 1);
        n[b] += 1;
        for (i, r) in rock.iter_mut().enumerate() {
            r[b] += truth >> i & 1;
        }
    }
    print!("  y ");
    BUCKETS.iter().for_each(|(l, h)| print!(" {:>6}", format!("{l}-{h}")));
    println!();
    for i in (0..rows).rev() {
        print!("{:>3} ", lo + i as i32);
        (0..nb).for_each(|b| print!(" {:>6.1}", 100.0 * rock[i][b] as f64 / n[b].max(1) as f64));
        println!();
    }
    print!("  n ");
    n.iter().for_each(|c| print!(" {c:>6}"));
    println!();
    Ok(())
}
