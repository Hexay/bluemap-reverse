//! Ground truth: structure starts a real 26.3 server located (fixtures/seed/numeric_seed.json, random_seed.json).

use bmr_cubiomes::{Generator, StructureType};

const MC: (u32, u32, u32) = (26, 3, 0);
const NUMERIC_SEED: i64 = 8675309867530986753;
const RANDOM_SEED: i64 = 6261841097431552160;

/// (seed, structure_set, start chunk) from the fixtures.
const LOCATED: &[(i64, &str, (i32, i32))] = &[
    (NUMERIC_SEED, "villages", (-271, -159)),
    (NUMERIC_SEED, "villages", (-231, -215)),
    (NUMERIC_SEED, "villages", (-231, 56)),
    (NUMERIC_SEED, "desert_pyramids", (76, -251)),
    (NUMERIC_SEED, "jungle_temples", (-255, 19)),
    (NUMERIC_SEED, "jungle_temples", (-188, 202)),
    (NUMERIC_SEED, "igloos", (-125, -117)),
    (RANDOM_SEED, "villages", (-230, -169)),
    (RANDOM_SEED, "villages", (-219, -61)),
    (RANDOM_SEED, "desert_pyramids", (-377, 0)),
    (RANDOM_SEED, "jungle_temples", (-346, -110)),
    (RANDOM_SEED, "swamp_huts", (-108, 41)),
];

fn structure(set: &str) -> StructureType {
    StructureType::for_set(set).unwrap_or_else(|| panic!("{set} has no biome check"))
}

fn seeded(seed: i64) -> Generator {
    let mut g = Generator::new(MC).expect("26.3 is supported");
    g.apply_seed(seed);
    g
}

#[test]
fn located_structures_are_viable_for_their_seed() {
    for &(seed, set, chunk) in LOCATED {
        assert!(seeded(seed).is_viable(structure(set), chunk), "{seed} {set} {chunk:?}");
    }
}

/// Jungle temples need jungle, desert pyramids desert, sampled at the same point: disjoint at a located temple.
#[test]
fn desert_pyramid_is_not_viable_at_a_jungle_temple() {
    let desert = structure("desert_pyramids");
    for &(seed, set, chunk) in LOCATED.iter().filter(|(_, set, _)| *set == "jungle_temples") {
        assert!(!seeded(seed).is_viable(desert, chunk), "{seed} {set} {chunk:?}");
    }
}

#[test]
fn reseeding_is_deterministic() {
    let village = structure("villages");
    let grid: Vec<(i32, i32)> = (-20..20).flat_map(|x| (-20..20).map(move |z| (x * 7, z * 7))).collect();
    let mut g = Generator::new(MC).unwrap();
    let mut answers = |seed| {
        g.apply_seed(seed);
        grid.iter().map(|&c| g.is_viable(village, c)).collect::<Vec<_>>()
    };
    let first = answers(NUMERIC_SEED);
    let other = answers(RANDOM_SEED);
    assert_eq!(answers(NUMERIC_SEED), first);
    assert_ne!(first, other);
    assert!(first.contains(&true) && first.contains(&false));
}

#[test]
fn unsupported_versions_are_rejected() {
    assert!(Generator::new((1, 17, 1)).is_none());
    assert!(Generator::new((0, 0, 0)).is_none());
    assert!(Generator::new((1, 18, 0)).is_some());
}

#[test]
fn sets_without_overworld_check_have_no_type() {
    assert!(StructureType::for_set("nether_complexes").is_none());
    assert!(StructureType::for_set("end_cities").is_none());
    assert!(StructureType::for_set("vill\0ages").is_none());
}

#[test]
fn unseeded_query_is_sound() {
    let mut g = Generator::new(MC).unwrap();
    assert!(!g.is_viable(structure("villages"), (0, 0)));
}
