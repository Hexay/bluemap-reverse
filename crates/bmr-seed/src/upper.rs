//! Upper 16 bits: of the 2^16 world seeds sharing a structure seed, keep those whose overworld biomes allow the
//! observed structures (cubiomes). Positions depend only on the lower 48 bits, viability on all 64.

use anyhow::{Context, Result};
use bmr_cubiomes::{Generator, StructureType};
use rayon::prelude::*;
use serde::Serialize;

use crate::observation::Constraint;
use crate::solve::matching_chunk;
use crate::structure_set::parse_version;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Ranked {
    pub seed: i64,
    /// observed structures the seed's biomes allow
    pub viable: usize,
}

/// A structure type at a start chunk, to test for biome viability.
pub type Check = (StructureType, (i32, i32));

/// Structures checked per seed. 20 already separate the true seed on the fixtures (docs/seed.md); more only
/// cost time, since every one is checked for each of the 2^16 seeds.
pub const MAX_CHECKS: usize = 64;

/// Up to `MAX_CHECKS` checkable structures, each at the start chunk `structure_seed` places it (observations it
/// doesn't place are skipped), round-robin over sets so no one biome dominates.
pub fn sample_checks(constraints: &[Constraint], structure_seed: u64) -> Vec<Check> {
    let mut by_set: Vec<(&str, Vec<Check>)> = Vec::new();
    for c in constraints {
        let Some(t) = StructureType::for_set(c.set.name) else { continue };
        let Some(chunk) = matching_chunk(structure_seed as i64, c) else { continue };
        match by_set.iter_mut().find(|(name, _)| *name == c.set.name) {
            Some((_, v)) => v.push((t, chunk)),
            None => by_set.push((c.set.name, vec![(t, chunk)])),
        }
    }
    let mut out = Vec::new();
    for i in 0.. {
        let before = out.len();
        out.extend(by_set.iter().filter_map(|(_, v)| v.get(i)).copied());
        if out.len() == before || out.len() >= MAX_CHECKS {
            break;
        }
    }
    out.truncate(MAX_CHECKS);
    out
}

/// World seeds over all 2^16 upper-bit values, best first, keeping those with at most `max_misses` of `checks`
/// not biome-viable.
pub fn rank_upper_bits(mc: &str, structure_seed: u64, checks: &[Check], max_misses: usize) -> Result<Vec<Ranked>> {
    let version = parse_version(mc).context("bad Minecraft version")?;
    Generator::new(version).with_context(|| format!("cubiomes does not support Minecraft {mc}"))?;
    let mut ranked: Vec<Ranked> = (0..1u64 << 16)
        .into_par_iter()
        .map_init(
            || Generator::new(version).expect("checked above"),
            |g, upper| {
                let seed = (upper << 48 | structure_seed) as i64;
                g.apply_seed(seed);
                let mut misses = 0;
                for &(t, chunk) in checks {
                    if !g.is_viable(t, chunk) {
                        misses += 1;
                        if misses > max_misses {
                            return None;
                        }
                    }
                }
                Some(Ranked { seed, viable: checks.len() - misses })
            },
        )
        .flatten()
        .collect();
    ranked.sort_by(|a, b| b.viable.cmp(&a.viable).then(a.seed.cmp(&b.seed)));
    Ok(ranked)
}
