//! Structure seed (lower 48 bits) → full 64-bit world seed, for the two ways worlds usually get seeds.
//! Anything else needs the 2^16 upper bits tested against biomes/terrain.

use serde::Serialize;

use crate::java_random::{MASK, step_back};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// blank seed field: `RandomSource.create().nextLong()`, a 48-bit LCG, so only 2^48 world seeds are reachable
    RandomSeed,
    /// non-numeric text: `String.hashCode()`, sign-extended to 64 bits
    TextSeed,
}

/// Most likely first: a structure seed that sign-extends from 32 bits by chance is a 1-in-2^15 event.
pub fn shortcuts(structure_seed: u64) -> Vec<(i64, Origin)> {
    let text = text_seed(structure_seed).map(|seed| (seed, Origin::TextSeed));
    let random = next_long_seeds(structure_seed).into_iter().map(|seed| (seed, Origin::RandomSeed));
    text.into_iter().chain(random).collect()
}

pub fn text_seed(structure_seed: u64) -> Option<i64> {
    let seed = structure_seed as u32 as i32 as i64;
    (seed as u64 & MASK == structure_seed).then_some(seed)
}

/// Every `nextLong()` output whose lower 48 bits are `structure_seed`. The low 32 bits of the output are the
/// second `next(32)`, i.e. bits 16..48 of the second LCG state, leaving 2^16 states to step back from.
pub fn next_long_seeds(structure_seed: u64) -> Vec<i64> {
    let lo_state_high = (structure_seed & 0xFFFF_FFFF) << 16;
    (0..1u64 << 16)
        .filter_map(|t| {
            let s2 = lo_state_high | t;
            let s1 = step_back(s2);
            let hi = (s1 >> 16) as u32 as i32 as i64;
            let lo = (s2 >> 16) as u32 as i32 as i64;
            let seed = (hi << 32).wrapping_add(lo);
            (seed as u64 & MASK == structure_seed).then_some(seed)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_random::{JavaRandom, string_hash};

    #[test]
    fn recovers_next_long() {
        for s in [0i64, 1, -7, 123456789, i64::MIN / 3] {
            let seed = JavaRandom::new(s).next_long();
            assert!(next_long_seeds(seed as u64 & MASK).contains(&seed), "{seed}");
        }
    }

    #[test]
    fn recovers_text() {
        let seed = string_hash("bluemap_reverse") as i64;
        assert_eq!(text_seed(seed as u64 & MASK), Some(seed));
    }
}
