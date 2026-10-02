use bmr_world::BlockInfo;

use super::*;
use crate::test_util::*;

const DIRT: &str = "minecraft:block/dirt";

#[test]
fn from_entries_resorts_signatures_with_their_uvs() {
    let sig = cube(STONE);
    let mut e = entry("minecraft:stone", sig.clone());
    e.sig.reverse();
    e.uvs = (0..6).map(|i| [[i, 0]; 4]).collect();
    let tagged: Vec<(FaceKey, Uv)> = e.sig.iter().copied().zip(e.uvs.iter().copied()).collect();
    let lib = library(vec![e]);
    assert_eq!(lib.exact(&sig), Some(&[0][..]));
    let after: Vec<(FaceKey, Uv)> =
        lib.entries[0].sig.iter().copied().zip(lib.entries[0].uvs.iter().copied()).collect();
    assert_eq!(signature_uv(tagged), signature_uv(after), "each uv stays with its key");
}

#[test]
fn indexes_by_state_texture_and_normal_form() {
    let mixed = cube_with(|d| if d.1 != 0 { DIRT } else { STONE });
    let lib = library(vec![
        entry("minecraft:stone", cube(STONE)),
        entry("minecraft:test_mixed", mixed),
        entry("minecraft:dirt", cube(DIRT)),
    ]);
    assert_eq!(lib.find(&state("minecraft:dirt")), Some(2));
    assert_eq!(lib.find(&state("minecraft:air")), None);
    assert_eq!(lib.with_textures([Tex::intern(STONE)]), [0, 1]);
    assert_eq!(lib.with_textures([Tex::intern(STONE), Tex::intern(DIRT)]), [1]);
    assert!(lib.with_textures([Tex::intern("minecraft:block/nowhere")]).is_empty());
    assert!(lib.with_textures([]).is_empty());
    assert_eq!(lib.exact_normalized(&normalized(&cube(DIRT))), Some(&[2][..]));
    assert_eq!((lib.stats.states, lib.data_version), (3, 0));
}

#[test]
fn overhang_only_entries_are_unmatchable_but_indexed() {
    let mut fire = entry("minecraft:fire", vec![]);
    fire.overhang = vec![((0, 1, 0), side("minecraft:block/fire_0", (0, -1, 0)))];
    let lib = library(vec![fire]);
    assert!(lib.entries.is_empty());
    assert_eq!(lib.overhang_from(&side("minecraft:block/fire_1", (0, -1, 0))), [(0, 1, 0)]);
    assert_eq!((lib.stats.overhang_faces, lib.stats.overhang_states), (1, 1));
}

#[test]
fn liquid_variant_flips_waterlogged_or_finds_same_faces() {
    let slab = vec![horizontal(STONE, 32), side(STONE, (0, -1, 0))];
    let mut wet = entry("minecraft:stone_slab[type=bottom,waterlogged=true]", slab.clone());
    wet.liquid = Some(Liquid::Water);
    let mut lava = entry("minecraft:lava_cauldron", cube(DIRT));
    lava.liquid = Some(Liquid::Lava);
    let lib = library(vec![
        entry("minecraft:stone_slab[type=bottom,waterlogged=false]", slab),
        wet,
        entry("minecraft:cauldron", cube(DIRT)),
        lava,
    ]);
    assert_eq!(lib.liquid_variant(0, Some(Liquid::Water)), Some(1));
    assert_eq!(lib.liquid_variant(1, None), Some(0));
    assert_eq!(lib.liquid_variant(0, None), None, "already fits");
    assert_eq!(lib.liquid_variant(2, Some(Liquid::Lava)), Some(3));
    assert_eq!(lib.liquid_variant(2, Some(Liquid::Water)), None);
}

#[test]
fn full_cube_needs_all_six_full_sides() {
    assert!(is_full_cube(&cube(STONE)));
    let mut five = cube(STONE);
    five.pop();
    assert!(!is_full_cube(&five));
    five.push(horizontal(STONE, 32));
    assert!(!is_full_cube(&five));
}

#[test]
fn default_distance_counts_changed_properties() {
    let info = BlockInfo {
        properties: vec![],
        default: vec![("facing".into(), "north".into()), ("lit".into(), "false".into())],
    };
    assert_eq!(default_distance(&state("minecraft:furnace[facing=north,lit=false]"), &info), 0);
    assert_eq!(default_distance(&state("minecraft:furnace[facing=east,lit=true]"), &info), 2);
}

#[test]
fn anchor_resolves_even_cells_to_nearer_odd_neighbour() {
    assert_eq!(axis_anchor(3, 3.5), 3);
    assert_eq!(axis_anchor(-1, -0.5), -1);
    assert_eq!(axis_anchor(2, 2.9), 3);
    assert_eq!(axis_anchor(2, 2.1), 1);
    assert_eq!(axis_anchor(2, 2.5), 1, "ties go to the lower block");
    assert_eq!(axis_anchor(-2, -1.2), -1);
}
