use super::*;

fn state(name: &str) -> BlockState {
    BlockState::new(format!("minecraft:{name}"), Vec::new())
}

/// (ylo, yhi, block name without namespace) per run in `column`, top first.
fn runs_of(runs: &[Run], column: Column) -> Vec<(i32, i32, String)> {
    let mut v: Vec<_> = runs
        .iter()
        .filter(|r| r.column == column)
        .map(|r| (r.ylo, r.yhi, r.state.name.trim_start_matches("minecraft:").to_owned()))
        .collect();
    v.sort_by_key(|r| -r.1);
    v
}

fn fill(
    gaps: &[SolidGap],
    profile: &Profile,
    cubes: &[((i32, i32, i32), BlockState)],
    rock_at: impl Fn(Column) -> Option<&'static str>,
) -> Vec<Run> {
    let registry = BlockRegistry::from_blocks(Vec::new());
    solid_segments(gaps, profile, &registry, cubes.iter().map(|(c, s)| (*c, s)), rock_at)
}

#[test]
fn end_spike_is_obsidian_down_to_the_floor() {
    let end = Profile::for_dimension("minecraft:the_end");
    let (obsidian, bedrock) = (state("obsidian"), state("bedrock"));
    let gaps = [
        SolidGap { column: (0, 0), ylo: 0, yhi: 79, above: Some(&bedrock) },
        SolidGap { column: (1, 0), ylo: 0, yhi: 79, above: Some(&obsidian) },
    ];
    let runs = fill(&gaps, &end, &[], |_| None);
    assert_eq!(runs_of(&runs, (1, 0)), [(0, 79, "obsidian".to_owned())]);
    // the pillar's bedrock cap is a build: its column copies the obsidian beside it
    assert_eq!(runs_of(&runs, (0, 0)), [(0, 79, "obsidian".to_owned())]);
}

#[test]
fn obsidian_outside_the_end_is_a_build() {
    let overworld = Profile::for_dimension("minecraft:overworld");
    let obsidian = state("obsidian");
    let gaps = [SolidGap { column: (0, 0), ylo: 10, yhi: 20, above: Some(&obsidian) }];
    assert_eq!(runs_of(&fill(&gaps, &overworld, &[], |_| None), (0, 0)), [(10, 20, "stone".to_owned())]);
}

#[test]
fn basalt_deltas_rock_replaces_netherrack_but_not_bedrock() {
    let nether = Profile::for_dimension("minecraft:the_nether");
    let netherrack = state("netherrack");
    let gaps = [SolidGap { column: (0, 0), ylo: 0, yhi: 30, above: Some(&netherrack) }];
    let runs = fill(&gaps, &nether, &[], |_| biome_rock("minecraft:basalt_deltas"));
    assert_eq!(runs_of(&runs, (0, 0)), [(3, 30, "basalt".to_owned()), (0, 2, "bedrock".to_owned())]);
}

#[test]
fn gap_under_a_build_copies_the_neighbouring_layers() {
    let overworld = Profile::for_dimension("minecraft:overworld");
    let (grass, wool) = (state("grass_block"), state("white_wool"));
    let gaps = [
        SolidGap { column: (0, 0), ylo: 50, yhi: 63, above: Some(&grass) },
        SolidGap { column: (1, 0), ylo: 50, yhi: 63, above: Some(&wool) },
    ];
    let runs = fill(&gaps, &overworld, &[], |_| None);
    let expect = [(61, 63, "dirt".to_owned()), (50, 60, "stone".to_owned())];
    assert_eq!(runs_of(&runs, (0, 0)), expect);
    assert_eq!(runs_of(&runs, (1, 0)), expect);
}

#[test]
fn seed_gradients_place_bedrock_per_cell() {
    let seeded = Profile::for_dimension("minecraft:overworld").with_seed(1_539_995_237);
    let stone = state("stone");
    let gaps: Vec<SolidGap> =
        (0..64).map(|x| SolidGap { column: (x, 0), ylo: -64, yhi: -50, above: Some(&stone) }).collect();
    let runs = fill(&gaps, &seeded, &[], |_| None);
    for x in 0..64 {
        for y in -64..=-50 {
            let r = runs.iter().find(|r| r.column == (x, 0) && (r.ylo..=r.yhi).contains(&y)).unwrap();
            assert_eq!(r.state.name, seeded.block_at(x, y, 0), "({x}, {y})");
        }
    }
    let bedrock_at =
        |y| runs.iter().filter(|r| r.state.name == "minecraft:bedrock" && r.ylo <= y && y <= r.yhi).count();
    assert_eq!(bedrock_at(-64), 64);
    assert!((1..64).contains(&bedrock_at(-61)), "the gradient band mixes bedrock and deepslate");
}
