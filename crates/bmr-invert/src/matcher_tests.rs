use super::*;
use crate::face::{Cell, signature};
use crate::test_util::*;

const DIRT: &str = "minecraft:block/dirt";
const GRASS: &str = "minecraft:block/short_grass";
const CANDLE: &str = "minecraft:block/candle";

fn without(sig: Vec<FaceKey>, drop: &[Cell]) -> Vec<FaceKey> {
    sig.into_iter().filter(|k| !drop.iter().any(|&d| k.boundary_dir() == Some(d))).collect()
}

fn ids(c: Option<Candidates>) -> Option<(Vec<usize>, How)> {
    c.map(|c| (c.ids, c.how))
}

#[test]
fn exact_signature() {
    let lib = library(vec![entry("minecraft:stone", cube(STONE)), entry("minecraft:dirt", cube(DIRT))]);
    assert_eq!(ids(candidates(&lib, &cube(DIRT))), Some((vec![id(&lib, "minecraft:dirt")], How::Exact)));
}

#[test]
fn offset_is_undone_before_matching() {
    let lib = library(vec![entry("minecraft:short_grass", cross(GRASS, 0, 0))]);
    let got = ids(candidates(&lib, &cross(GRASS, 5, -3)));
    assert_eq!(got, Some((vec![0], How::Offset)));
}

#[test]
fn offset_tolerates_one_unit_rounding_jitter() {
    let lib = library(vec![entry("minecraft:short_grass", cross(GRASS, 0, 0))]);
    let mut sig = cross(GRASS, 5, -3);
    sig[0].verts[1][1] -= 1;
    let sig = signature(sig);
    assert!(lib.exact_normalized(&normalized(&sig)).is_none());
    assert_eq!(ids(candidates(&lib, &sig)), Some((vec![0], How::Offset)));
}

#[test]
fn offset_jitter_beyond_one_unit_fails() {
    let lib = library(vec![entry("minecraft:short_grass", cross(GRASS, 0, 0))]);
    let mut sig = cross(GRASS, 5, -3);
    sig[0].verts[1][1] -= 2;
    assert_eq!(ids(candidates(&lib, &signature(sig))), None);
}

#[test]
fn partial_prefers_fewest_missing_boundary_faces() {
    let lib = library(vec![
        entry("minecraft:stone", cube(STONE)),
        entry("minecraft:test_open_top", without(cube(STONE), &[(0, 1, 0)])),
    ]);
    let walls = without(cube(STONE), &[(0, 1, 0), (0, -1, 0)]);
    assert_eq!(ids(candidates(&lib, &walls)), Some((vec![id(&lib, "minecraft:test_open_top")], How::Partial)));

    // the open-top entry lacks the observed top: an extra face rules it out
    let side_culled = without(cube(STONE), &[(1, 0, 0)]);
    let stone = vec![id(&lib, "minecraft:stone")];
    assert_eq!(ids(candidates(&lib, &side_culled)), Some((stone, How::Partial)));
}

#[test]
fn partial_returns_every_equally_good_entry() {
    let lib = library(vec![
        entry("minecraft:test_a", without(cube(STONE), &[(0, 1, 0)])),
        entry("minecraft:test_b", without(cube(STONE), &[(0, -1, 0)])),
    ]);
    let walls = without(cube(STONE), &[(0, 1, 0), (0, -1, 0)]);
    assert_eq!(ids(candidates(&lib, &walls)), Some((vec![0, 1], How::Partial)));
}

#[test]
fn extra_or_missing_interior_faces_never_match() {
    let lib = library(vec![entry("minecraft:stone", cube(STONE)), entry("minecraft:short_grass", cross(GRASS, 0, 0))]);
    let mut extra = cube(STONE);
    extra.push(horizontal(STONE, 32));
    assert!(candidates(&lib, &signature(extra)).is_none());
    assert!(candidates(&lib, &cross(GRASS, 0, 0)[..1]).is_none());
    assert!(candidates(&lib, &cube("minecraft:block/not_in_library")).is_none());
}

#[test]
fn cullable_interior_faces_only_in_relaxed_pass() {
    let candle = vec![side(CANDLE, (1, 0, 0)), side(CANDLE, (-1, 0, 0)), horizontal(CANDLE, 32)];
    let lib = library(vec![entry("minecraft:candle", candle)]);
    let seen = signature(vec![side(CANDLE, (1, 0, 0)), side(CANDLE, (-1, 0, 0))]);
    assert!(candidates(&lib, &seen).is_none(), "strict pass may only miss boundary faces");
    assert_eq!(ids(candidates_cullable(&lib, &seen)), Some((vec![0], How::Partial)));

    // one matched face cannot outweigh a missing cullable one: could be a neighbour's overhang
    assert!(candidates_cullable(&lib, &[side(CANDLE, (1, 0, 0))]).is_none());
}

fn obs_of(faces: &[crate::face::WorldFace]) -> CellFaces {
    let mut c = CellFaces::default();
    for f in faces {
        c.push(f, (0, 0, 0));
    }
    c
}

#[test]
fn resolve_falls_back_to_default_distance_then_id() {
    let mut e = vec![
        entry("minecraft:test[power=0]", cube(STONE)),
        entry("minecraft:test[power=1]", cube(STONE)),
        entry("minecraft:test[power=2]", cube(STONE)),
    ];
    (e[0].default_distance, e[1].default_distance, e[2].default_distance) = (2, 1, 1);
    let lib = library(e);
    assert_eq!(resolve(&lib, &[0, 1, 2], &CellFaces::default()), 1);
    assert_eq!(resolve(&lib, &[2], &CellFaces::default()), 2);
}

#[test]
fn resolve_picks_nearest_tint_first() {
    let wire = horizontal("minecraft:block/redstone_dust_dot", 1);
    let mut e = vec![
        entry("minecraft:redstone_wire[power=0]", vec![wire]),
        entry("minecraft:redstone_wire[power=15]", vec![wire]),
    ];
    (e[0].tint, e[1].tint) = (Some([75, 0, 0]), Some([252, 50, 0]));
    e[1].default_distance = 1;
    let lib = library(e);
    let up = (0, 1, 0);
    let bright = obs_of(&[world_side((0, 0, 0), up, STONE, [250, 49, 0], 0)]);
    assert_eq!(resolve(&lib, &[0, 1], &bright), 1);
    let dim = obs_of(&[world_side((0, 0, 0), up, STONE, [80, 0, 0], 0)]);
    assert_eq!(resolve(&lib, &[0, 1], &dim), 0);
    assert_eq!(resolve(&lib, &[0, 1], &CellFaces::default()), 0, "untinted observation: tint does not decide");
}

#[test]
fn resolve_uses_uvs_only_for_orientation_differences() {
    const DOOR: &str = "minecraft:block/oak_door_bottom";
    let face = world_side((0, 0, 0), (1, 0, 0), DOOR, [255; 3], 0);
    let (k, uv) = face.key_uv_at((0, 0, 0));
    let mirrored = [uv[1], uv[0], uv[3], uv[2]];
    let with_uv = |s: &str, uv, dd| Entry { uvs: vec![uv], default_distance: dd, ..entry(s, vec![k]) };
    let obs = obs_of(&[face]);

    let doors = library(vec![
        with_uv("minecraft:oak_door[hinge=left]", mirrored, 0),
        with_uv("minecraft:oak_door[hinge=right]", uv, 1),
    ]);
    assert_eq!(resolve(&doors, &[0, 1], &obs), 1);

    // position-hash rotation: UVs differ by chance, not by property
    let stones = library(vec![with_uv("minecraft:stone", mirrored, 0), with_uv("minecraft:infested_stone", uv, 0)]);
    assert_eq!(resolve(&stones, &[0, 1], &obs), 0);
}

#[test]
fn resolve_uses_light_only_when_lit_differs() {
    const FRONT: &str = "minecraft:block/furnace_front";
    let k = side(FRONT, (1, 0, 0));
    let mut lit = entry("minecraft:furnace[lit=true]", vec![k]);
    (lit.light, lit.default_distance) = (13, 1);
    let lib = library(vec![entry("minecraft:furnace[lit=false]", vec![k]), lit]);
    let glow = |l| obs_of(&[world_side((0, 0, 0), (1, 0, 0), FRONT, [255; 3], l)]);
    assert_eq!(resolve(&lib, &[0, 1], &glow(13)), 1);
    assert_eq!(resolve(&lib, &[0, 1], &glow(0)), 0);
}
