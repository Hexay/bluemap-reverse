//! Against real servers' /locate output (tools/structure_truth.py → fixtures/seed/*.json).

use bmr_seed::java_random::MASK;
use bmr_seed::observation::constraints;
use bmr_seed::placement::{candidate_chunk, passes_reducer, region_of};
use bmr_seed::upper::{rank_upper_bits, sample_checks};
use bmr_seed::{Observation, crack};
use serde::Deserialize;

#[derive(Deserialize)]
struct Truth {
    mc: String,
    seed: String,
    structures: Vec<Observation>,
}

fn truths() -> Vec<(String, Truth)> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/seed");
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name, serde_json::from_slice(&std::fs::read(e.path()).unwrap()).unwrap())
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!out.is_empty(), "no fixtures in {dir}");
    out
}

#[test]
fn forward_model_predicts_every_located_structure() {
    for (name, t) in truths() {
        let seed: i64 = t.seed.parse().unwrap();
        for c in constraints(&t.mc, &t.structures).unwrap() {
            let chunk = c.options[0].chunk;
            let (rx, rz) = region_of(&c.set, chunk);
            assert_eq!(candidate_chunk(seed, &c.set, rx, rz), chunk, "{name}: {}", c.set.name);
            assert!(passes_reducer(seed, &c.set, chunk), "{name}: {} reducer", c.set.name);
        }
    }
}

fn assert_cracks(name: &str, t: &Truth, observations: &[Observation], max_misses: usize) {
    let seed: i64 = t.seed.parse().unwrap();
    let report = crack(&t.mc, observations, max_misses).unwrap();
    let top = report.candidates.first().unwrap_or_else(|| panic!("{name}: no candidates"));
    assert_eq!(top.structure_seed, seed as u64 & MASK, "{name}: {report:?}");
    assert!(report.unique, "{name}: {} candidates tie", report.candidates.len());
    assert_eq!(report.world_seed, Some(seed), "{name}: {:?}", top.world_seeds);
}

/// Without the shortcuts' help: biome viability of 20 structures alone ranks the true seed first, untied.
#[test]
fn biomes_alone_pick_the_world_seed() {
    for (name, t) in truths() {
        let seed: i64 = t.seed.parse().unwrap();
        let checks = sample_checks(&constraints(&t.mc, &t.structures).unwrap(), seed as u64 & MASK);
        let ranked = rank_upper_bits(&t.mc, seed as u64 & MASK, &checks[..20], 20).unwrap();
        assert_eq!(ranked[0].seed, seed, "{name}");
        assert!(ranked[0].viable > ranked[1].viable, "{name}: {:?}", &ranked[..3]);
    }
}

#[test]
fn cracks_from_all_structures() {
    for (name, t) in truths() {
        assert_cracks(&name, &t, &t.structures, 1);
    }
}

#[test]
/// One set alone plateaus at a few tied candidates (same range → correlated offsets); mixed ranges resolve fast.
fn cracks_from_twelve_mixed() {
    let sets = ["desert_pyramids", "jungle_temples", "swamp_huts", "igloos", "shipwrecks", "villages"];
    for (name, t) in truths() {
        let mut few: Vec<Observation> = Vec::new();
        for set in sets {
            few.extend(t.structures.iter().filter(|o| o.set == set).take(2).cloned());
        }
        assert_cracks(&name, &t, &few, 0);
    }
}

/// A typical small server map: 2048×2048 blocks around spawn.
#[test]
fn cracks_from_a_small_map() {
    for (name, t) in truths() {
        let near: Vec<_> = t.structures.iter().filter(|o| o.chunks[0].iter().all(|c| c.abs() < 64)).cloned().collect();
        eprintln!("{name}: {} structures within 1024 blocks", near.len());
        assert_cracks(&name, &t, &near, 1);
    }
}

#[test]
fn survives_misidentified_structures() {
    for (name, t) in truths() {
        let mut obs = t.structures.clone();
        for o in obs.iter_mut().step_by(10).take(2) {
            o.chunks[0][0] += 1;
        }
        assert_cracks(&name, &t, &obs, 2);
    }
}
