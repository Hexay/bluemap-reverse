//! Structure seed (lower 48 bits) from constraints, tolerating up to `max_misses` wrong observations.
//!
//! Lifting: for a linear set with range r = 2^k·m (m odd, r not a power of two), `nextInt(r) mod 2^k` equals
//! bits [17, 17+k) of the LCG state, which depend only on `seed mod 2^(17+k)`. So the low 17+K bits are
//! enumerated and filtered first, then the upper bits per survivor against every constraint.

use anyhow::{Result, bail};
use rayon::prelude::*;

use crate::java_random::{MULT, step};
use crate::observation::Constraint;
use crate::placement::{candidate_offset, passes_reducer};

/// Upper-bit pass budget (survivors × 2^(48−low bits)); ~2^28 checks/s/core, so this is about a minute.
const MAX_WORK: u64 = 1 << 34;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub structure_seed: u64,
    pub matched: usize,
}

pub struct Solution {
    pub candidates: Vec<Candidate>,
    pub low_bits: u32,
    pub low_survivors: usize,
}

pub fn solve(constraints: &[Constraint], max_misses: usize) -> Result<Solution> {
    let liftable: Vec<&Constraint> = constraints.iter().filter(|c| c.lift > 0 && c.feasible()).collect();
    let infeasible = constraints.iter().filter(|c| !c.feasible()).count();
    if infeasible > max_misses {
        bail!("{infeasible} observations are impossible for their structure set (> {max_misses} allowed misses)");
    }
    let budget = max_misses - infeasible;
    if liftable.len() < 2 {
        bail!(
            "need at least 2 liftable structures (temples, huts, igloos, outposts, shipwrecks, ocean ruins, villages)"
        );
    }
    let low_bits = 17 + liftable.iter().map(|c| c.lift).max().unwrap_or(0);

    let lows: Vec<u64> = (0..1u64 << low_bits)
        .into_par_iter()
        .filter(|&low| liftable.iter().filter(|c| !low_bits_match(low, c)).nth(budget).is_none())
        .collect();
    let high_count = 1u64 << (48 - low_bits);
    if lows.len() as u64 * high_count > MAX_WORK {
        bail!(
            "{} low-bit candidates × 2^{}: add more temples/huts/igloos/outposts/shipwrecks or lower --max-misses",
            lows.len(),
            48 - low_bits
        );
    }

    let feasible: Vec<&Constraint> = constraints.iter().filter(|c| c.feasible()).collect();
    let mut candidates: Vec<Candidate> = lows
        .iter()
        .flat_map(|&low| {
            let feasible = &feasible;
            (0..high_count)
                .into_par_iter()
                .filter_map(move |high| {
                    let seed = high << low_bits | low;
                    let mut misses = 0;
                    for c in feasible {
                        if !matches(seed as i64, c) {
                            misses += 1;
                            if misses > budget {
                                return None;
                            }
                        }
                    }
                    Some(Candidate { structure_seed: seed, matched: feasible.len() - misses })
                })
                .collect::<Vec<_>>()
        })
        .collect();
    candidates.sort_by(|a, b| b.matched.cmp(&a.matched).then(a.structure_seed.cmp(&b.structure_seed)));
    Ok(Solution { candidates, low_bits, low_survivors: lows.len() })
}

pub fn matches(seed: i64, c: &Constraint) -> bool {
    matching_chunk(seed, c).is_some()
}

/// The option the seed would place this structure at, if any.
pub fn matching_chunk(seed: i64, c: &Constraint) -> Option<(i32, i32)> {
    c.options
        .iter()
        .find(|p| candidate_offset(seed, p.offset, &c.set) == p.target && passes_reducer(seed, &c.set, p.chunk))
        .map(|p| p.chunk)
}

/// Some option's low `lift` bits per axis match, from `seed mod 2^(17+lift)` alone.
fn low_bits_match(low: u64, c: &Constraint) -> bool {
    let bits = 17 + c.lift;
    let mask = (1u64 << bits) - 1;
    let keep = (1i32 << c.lift) - 1;
    c.options.iter().any(|p| {
        let s1 = step((low.wrapping_add(p.offset as u64) ^ MULT) & mask) & mask;
        let s2 = step(s1) & mask;
        ((s1 >> 17) as i32 & keep) == (p.target.0 & keep) && ((s2 >> 17) as i32 & keep) == (p.target.1 & keep)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::{Observation, constraints};
    use crate::placement::candidate_chunk;
    use crate::structure_set::vanilla;

    /// Distinct regions per set: sets with consecutive salts (the temples) in the same region are nearly
    /// redundant, since the region seeds differ by 1.
    fn synthetic(seed: i64, sets: &[&str], per_set: i32) -> Vec<Constraint> {
        let all = vanilla("26.3").unwrap();
        let mut obs = Vec::new();
        for (i, name) in sets.iter().enumerate() {
            let set = all.iter().find(|s| s.name == *name).unwrap();
            for j in 0..per_set {
                let (cx, cz) = candidate_chunk(seed, set, i as i32 * 7 - 9, j * 5 - 4);
                obs.push(Observation { set: name.to_string(), chunks: vec![[cx, cz]] });
            }
        }
        constraints("26.3", &obs).unwrap()
    }

    #[test]
    fn low_bits_agree_with_full_placement() {
        let seed = 0x1234_5678_9ABCi64;
        for c in synthetic(seed, &["desert_pyramids", "shipwrecks", "villages"], 3) {
            assert!(matches(seed, &c));
            assert!(low_bits_match(seed as u64 & ((1 << (17 + c.lift)) - 1), &c), "{}", c.set.name);
        }
    }

    #[test]
    fn ambiguous_observations_still_crack() {
        let seed = 0x0000_1234_ABCD_5678i64;
        let mut c = synthetic(seed, &["desert_pyramids", "igloos", "shipwrecks", "villages", "swamp_huts"], 3);
        for (i, k) in c.iter_mut().enumerate() {
            // a decoy neighbour chunk per observation, listed before or after the true one
            let p = k.options[0];
            let decoy = constraints(
                "26.3",
                &[Observation { set: k.set.name.into(), chunks: vec![[p.chunk.0 + 1, p.chunk.1]] }],
            );
            if let Some(d) = decoy.unwrap()[0].options.first().copied() {
                if i % 2 == 0 { k.options.insert(0, d) } else { k.options.push(d) }
            }
        }
        let s = solve(&c, 0).unwrap();
        assert_eq!(s.candidates[0].structure_seed, seed as u64 & crate::java_random::MASK);
        assert!(c.iter().all(|k| matching_chunk(seed, k).is_some()));
    }

    #[test]
    fn cracks_synthetic_mix() {
        let seed = 0x0000_BEEF_CAFE_F00Di64;
        let c = synthetic(seed, &["desert_pyramids", "igloos", "shipwrecks", "villages"], 3);
        let s = solve(&c, 0).unwrap();
        assert_eq!(s.candidates.len(), 1, "{:?}", &s.candidates[..s.candidates.len().min(8)]);
        assert_eq!(s.candidates[0].structure_seed, seed as u64 & crate::java_random::MASK);
    }
}
