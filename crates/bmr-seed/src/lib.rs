//! World-seed recovery from what a map shows (docs/seed.md): structure starts → structure seed (lower 48 bits)
//! → world seed (upper 16 bits by structure biome viability, ties broken by the random/text-seed shortcuts).

pub mod detect;
pub mod java_random;
pub mod observation;
pub mod placement;
pub mod solve;
pub mod structure_set;
pub mod upper;
pub mod world_seed;

use anyhow::Result;
use serde::Serialize;

pub use observation::Observation;
pub use world_seed::Origin;

/// Structure seeds taken to the upper-bit stage: the best-matching tier, capped (each costs ~1–2 s).
const MAX_STRUCTURE_SEEDS: usize = 4;
/// World seeds listed per structure seed.
const MAX_WORLD_SEEDS: usize = 5;

#[derive(Debug, Serialize)]
pub struct Report {
    pub observations: usize,
    /// upper bound on what the observations say about the 48-bit structure seed; one set alone saturates well
    /// below it (docs/seed.md), so `unique` is the real test
    pub bits: f64,
    /// the best structure seed explains strictly more observations than any other
    pub unique: bool,
    pub max_misses: usize,
    pub low_bits: u32,
    pub low_survivors: usize,
    pub candidates: Vec<CandidateReport>,
    /// the answer, when one world seed beats every other
    pub world_seed: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CandidateReport {
    pub structure_seed: u64,
    pub matched: usize,
    /// structures whose biome viability was checked per world seed
    pub biome_checks: usize,
    /// best first; empty when not taken to the upper-bit stage
    pub world_seeds: Vec<WorldSeed>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct WorldSeed {
    pub seed: i64,
    /// structures (of `CandidateReport::biome_checks`) the seed's biomes allow
    pub viable: usize,
    /// the shortcut this seed also fits, if any
    pub origin: Option<Origin>,
}

/// Default tolerance: one misidentified structure per six observations.
pub fn default_max_misses(observations: usize) -> usize {
    observations / 6
}

pub fn crack(mc: &str, observations: &[Observation], max_misses: usize) -> Result<Report> {
    let constraints = observation::constraints(mc, observations)?;
    let solution = solve::solve(&constraints, max_misses)?;
    let best = solution.candidates.first().map_or(0, |c| c.matched);
    let mut candidates = Vec::new();
    for (i, c) in solution.candidates.iter().enumerate() {
        let checks = if c.matched == best && i < MAX_STRUCTURE_SEEDS {
            upper::sample_checks(&constraints, c.structure_seed)
        } else {
            Vec::new()
        };
        let world_seeds = if checks.is_empty() {
            Vec::new()
        } else {
            world_seeds(mc, c.structure_seed, &checks, default_max_misses(checks.len()))?
        };
        candidates.push(CandidateReport {
            structure_seed: c.structure_seed,
            matched: c.matched,
            biome_checks: checks.len(),
            world_seeds,
        });
    }
    candidates.sort_by_key(|c| std::cmp::Reverse((c.matched, c.world_seeds.first().map_or(0, |w| w.viable))));
    Ok(Report {
        observations: constraints.len(),
        bits: observation::total_bits(&constraints),
        unique: match solution.candidates.as_slice() {
            [] => false,
            [_] => true,
            [a, b, ..] => a.matched > b.matched,
        },
        max_misses,
        low_bits: solution.low_bits,
        low_survivors: solution.low_survivors,
        world_seed: decide(&candidates),
        candidates,
    })
}

/// Biome-ranked world seeds; among equally viable ones, those fitting a shortcut first.
fn world_seeds(mc: &str, structure_seed: u64, checks: &[upper::Check], max_misses: usize) -> Result<Vec<WorldSeed>> {
    let shortcuts = world_seed::shortcuts(structure_seed);
    let origin = |seed| shortcuts.iter().find(|(s, _)| *s == seed).map(|&(_, o)| o);
    let mut ranked: Vec<WorldSeed> = upper::rank_upper_bits(mc, structure_seed, checks, max_misses)?
        .into_iter()
        .map(|r| WorldSeed { seed: r.seed, viable: r.viable, origin: origin(r.seed) })
        .collect();
    ranked.sort_by_key(|w| (std::cmp::Reverse(w.viable), w.origin.is_none()));
    ranked.truncate(MAX_WORLD_SEEDS);
    Ok(ranked)
}

/// A world seed that beats every other: more viable structures, or equally viable but the only shortcut fit.
fn decide(candidates: &[CandidateReport]) -> Option<i64> {
    let mut all: Vec<&WorldSeed> = candidates.iter().flat_map(|c| &c.world_seeds).collect();
    all.sort_by_key(|w| (std::cmp::Reverse(w.viable), w.origin.is_none()));
    match all.as_slice() {
        [] => None,
        [only] => Some(only.seed),
        [a, b, ..] => (a.viable > b.viable || (a.origin.is_some() && b.origin.is_none())).then_some(a.seed),
    }
}
