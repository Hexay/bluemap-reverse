use crate::face::Uv;
use crate::library::{Entry, Library};
use crate::test_util::*;

const UV_A: Uv = [[0, 0], [0, 256], [256, 0], [256, 256]];
const UV_B: Uv = [[256, 0], [256, 256], [0, 0], [0, 256]];

fn e(state: &str, uv: Uv) -> Entry {
    let k = side(STONE, (1, 0, 0));
    Entry { uvs: vec![uv], ..entry(state, vec![k]) }
}

fn groups(lib: &Library) -> Vec<Vec<String>> {
    let mut g: Vec<Vec<String>> = lib
        .lookalikes()
        .into_iter()
        .map(|ids| {
            let mut names: Vec<String> = ids.iter().map(|&i| lib.entries[i].state.to_string()).collect();
            names.sort();
            names
        })
        .collect();
    g.sort();
    g
}

#[test]
fn identical_renders_group_even_with_chance_uvs() {
    let lib = library(vec![e("minecraft:stone", UV_A), e("minecraft:infested_stone", UV_B)]);
    assert_eq!(groups(&lib), [["minecraft:infested_stone", "minecraft:stone"]]);
    assert!(!lib.tell_apart(0, 1));
}

#[test]
fn orientation_uvs_tell_apart_only_when_they_differ() {
    let lib = library(vec![
        e("minecraft:oak_door[hinge=left]", UV_A),
        e("minecraft:oak_door[hinge=right]", UV_B),
        e("minecraft:test[facing=north]", UV_A),
        e("minecraft:test[facing=south]", UV_A),
    ]);
    assert!(lib.tell_apart(0, 1));
    assert!(!lib.tell_apart(2, 3));
    // first-class membership: everything not distinguishable from door[hinge=left] joins it
    assert_eq!(
        groups(&lib),
        [vec!["minecraft:oak_door[hinge=left]", "minecraft:test[facing=north]", "minecraft:test[facing=south]"]]
    );
}

#[test]
fn light_tells_lit_states_apart() {
    let mut lit = e("minecraft:furnace[lit=true]", UV_A);
    lit.light = 13;
    let lib = library(vec![e("minecraft:furnace[lit=false]", UV_A), lit]);
    assert!(lib.tell_apart(0, 1));
    assert!(groups(&lib).is_empty());

    let lib = library(vec![e("minecraft:furnace[lit=false]", UV_A), e("minecraft:furnace[lit=true]", UV_A)]);
    assert_eq!(groups(&lib).len(), 1, "same light: the tile cannot tell");
}

#[test]
fn tint_liquid_and_overhang_split_groups() {
    let mut tinted = e("minecraft:test_tinted", UV_A);
    tinted.tint = Some([1, 2, 3]);
    let mut wet = e("minecraft:test_wet", UV_A);
    wet.liquid = Some(crate::face::Liquid::Water);
    let mut overhanging = e("minecraft:test_overhang", UV_A);
    overhanging.overhang = vec![((0, 1, 0), side(STONE, (0, -1, 0)))];
    let lib = library(vec![e("minecraft:test_plain", UV_A), tinted, wet, overhanging]);
    assert!(groups(&lib).is_empty());
}

#[test]
fn different_faces_never_group() {
    let lib =
        library(vec![entry("minecraft:stone", cube(STONE)), entry("minecraft:dirt", cube("minecraft:block/dirt"))]);
    assert!(groups(&lib).is_empty());
}
