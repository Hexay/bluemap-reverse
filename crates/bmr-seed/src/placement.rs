//! `RandomSpreadStructurePlacement`: each `spacing`² region holds one candidate start chunk, drawn from a
//! `java.util.Random` seeded by the structure seed (lower 48 bits), the region and the set's salt.

use crate::java_random::JavaRandom;
use crate::structure_set::{Reducer, Spread, StructureSet};

const REGION_X: i64 = 341873128712;
const REGION_Z: i64 = 132897987541;

/// Everything added to the world seed before `setSeed`; independent of the seed, so solvers precompute it.
pub fn region_offset(set: &StructureSet, rx: i32, rz: i32) -> i64 {
    (rx as i64).wrapping_mul(REGION_X).wrapping_add((rz as i64).wrapping_mul(REGION_Z)).wrapping_add(set.salt as i64)
}

pub fn region_of(set: &StructureSet, chunk: (i32, i32)) -> (i32, i32) {
    (chunk.0.div_euclid(set.spacing), chunk.1.div_euclid(set.spacing))
}

/// Candidate offset within the region, given `region_offset`.
pub fn candidate_offset(seed: i64, offset: i64, set: &StructureSet) -> (i32, i32) {
    let mut r = JavaRandom::new(seed.wrapping_add(offset));
    let range = set.range();
    match set.spread {
        Spread::Linear => (r.next_int(range), r.next_int(range)),
        Spread::Triangular => {
            let x = (r.next_int(range) + r.next_int(range)) / 2;
            (x, (r.next_int(range) + r.next_int(range)) / 2)
        }
    }
}

pub fn candidate_chunk(seed: i64, set: &StructureSet, rx: i32, rz: i32) -> (i32, i32) {
    let (x, z) = candidate_offset(seed, region_offset(set, rx, rz), set);
    (rx * set.spacing + x, rz * set.spacing + z)
}

/// The frequency reduction a candidate must also pass (always true for sets without one).
pub fn passes_reducer(seed: i64, set: &StructureSet, chunk: (i32, i32)) -> bool {
    match set.reducer {
        None => true,
        Some(Reducer::LegacyType1 { one_in }) => {
            let mix = (chunk.0 >> 4) ^ ((chunk.1 >> 4) << 4);
            let mut r = JavaRandom::new(mix as i64 ^ seed);
            r.next(32);
            r.next_int(one_in) == 0
        }
    }
}
