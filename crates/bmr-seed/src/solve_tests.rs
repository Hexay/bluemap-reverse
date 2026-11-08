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
        let decoy =
            constraints("26.3", &[Observation { set: k.set.name.into(), chunks: vec![[p.chunk.0 + 1, p.chunk.1]] }]);
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
