use rustc_hash::FxHashMap;

use super::*;
use crate::face::Cell;
use crate::test_util::*;

const SIGN: &str = "minecraft:block/oak_planks";
const EAST: Cell = (1, 0, 0);

/// A sign at the origin cell whose board reaches into the cell east of it.
fn board() -> FaceKey {
    side(SIGN, (-1, 0, 0))
}

fn lib() -> Library {
    let mut sign = entry("minecraft:oak_sign", vec![horizontal(SIGN, 32)]);
    sign.overhang = vec![(EAST, board())];
    library(vec![sign, entry("minecraft:stone", cube(STONE))])
}

fn stone_with_board() -> Vec<FaceKey> {
    let mut keys = cube(STONE);
    keys.push(board());
    keys
}

fn setup(cells: Vec<(Cell, Vec<FaceKey>)>, lib: &Library) -> (FxHashMap<Cell, CellFaces>, FxHashMap<Cell, Matched>) {
    let cells: FxHashMap<Cell, CellFaces> = cells.into_iter().map(|(c, k)| (c, observed(k))).collect();
    let matched = cells
        .iter()
        .map(|(&c, obs)| (c, candidates(lib, &signature(obs.keys.clone())).map(|m| (resolve(lib, &m.ids, obs), m.how))))
        .collect();
    (cells, matched)
}

#[test]
fn library_indexes_overhang_sources() {
    assert_eq!(lib().overhang_from(&board()), [EAST]);
    assert!(lib().overhang_from(&side(STONE, (1, 0, 0))).is_empty());
}

#[test]
fn credit_strips_overhang_and_rematches_neighbour() {
    let lib = lib();
    let (mut cells, mut matched) =
        setup(vec![((0, 0, 0), vec![horizontal(SIGN, 32)]), (EAST, stone_with_board())], &lib);
    assert_eq!(matched[&EAST], None, "the board is an extra face for stone");
    assert_eq!(credit(&lib, &mut cells, &mut matched), 0);
    assert_eq!(matched[&EAST], Some((id(&lib, "minecraft:stone"), How::Exact)));
    assert_eq!(cells[&EAST].keys.len(), 6);
}

#[test]
fn credit_removes_cells_holding_only_overhang() {
    let lib = lib();
    let (mut cells, mut matched) = setup(vec![((0, 0, 0), vec![horizontal(SIGN, 32)]), (EAST, vec![board()])], &lib);
    assert_eq!(credit(&lib, &mut cells, &mut matched), 1);
    assert!(!cells.contains_key(&EAST) && !matched.contains_key(&EAST));
    assert_eq!(matched[&(0, 0, 0)], Some((id(&lib, "minecraft:oak_sign"), How::Exact)));
}

/// A floor fire draws its side planes on the cell boundary; the one facing back in lands in the air cell
/// beside it, which on its own matches a wall fire whose model overhangs back into the floor fire's cell.
#[test]
fn a_cell_made_of_overhang_does_not_strip_its_source() {
    const FIRE: &str = "minecraft:block/fire_0";
    let east_plane = side(FIRE, (1, 0, 0));
    let in_east_cell = side(FIRE, (-1, 0, 0));
    let mut floor = entry("minecraft:fire", vec![horizontal(FIRE, 32), east_plane]);
    floor.overhang = vec![(EAST, in_east_cell)];
    let mut wall = entry("minecraft:fire[west=true]", vec![in_east_cell]);
    wall.overhang = vec![((-1, 0, 0), east_plane)];
    let lib = library(vec![floor, wall]);
    let (mut cells, mut matched) =
        setup(vec![((0, 0, 0), vec![horizontal(FIRE, 32), east_plane]), (EAST, vec![in_east_cell])], &lib);
    assert_eq!(credit(&lib, &mut cells, &mut matched), 1);
    assert!(!cells.contains_key(&EAST));
    assert_eq!(cells[&(0, 0, 0)].keys.len(), 2, "the floor fire keeps its side");
    assert_eq!(matched[&(0, 0, 0)], Some((id(&lib, "minecraft:fire"), How::Exact)));
}

/// Same, but the floor fire is still unmatched (a second fire beside it doubled one of its planes): the wall
/// fire's faces are explained by that unmatched neighbour, so it still may not strip the floor fire.
#[test]
fn overhang_from_an_unmatched_neighbour_does_not_claim_either() {
    const FIRE: &str = "minecraft:block/fire_0";
    let east_plane = side(FIRE, (1, 0, 0));
    let west_plane = side(FIRE, (-1, 0, 0));
    let mut floor = entry("minecraft:fire", vec![horizontal(FIRE, 32), east_plane]);
    floor.overhang = vec![(EAST, west_plane)];
    let mut wall = entry("minecraft:fire[west=true]", vec![west_plane]);
    wall.overhang = vec![((-1, 0, 0), east_plane)];
    let lib = library(vec![floor, wall]);
    let doubled = vec![horizontal(FIRE, 32), east_plane, side(FIRE, (0, 0, 1))];
    let (mut cells, mut matched) = setup(vec![((0, 0, 0), doubled), (EAST, vec![west_plane])], &lib);
    assert_eq!(matched[&(0, 0, 0)], None);
    credit(&lib, &mut cells, &mut matched);
    assert!(cells[&(0, 0, 0)].keys.contains(&east_plane), "the floor fire keeps its side");
}

/// A see-through block (spawner) draws its inner faces into every neighbour; each lone inner face matches the
/// block again and claims one of the real block's faces back. The block with more faces is the real one.
#[test]
fn ring_of_inner_faces_does_not_strip_the_real_block() {
    const CAGE: &str = "minecraft:block/spawner";
    let up = (0, 1, 0);
    let mut spawner = entry("minecraft:spawner", vec![horizontal(CAGE, 64), side(CAGE, (1, 0, 0))]);
    spawner.overhang = vec![(up, horizontal(CAGE, 0))];
    // the lone face above also reads as a spawner whose inner bottom face fell into the cell below
    let mut lone = entry("minecraft:trial_spawner", vec![horizontal(CAGE, 0)]);
    lone.overhang = vec![((0, -1, 0), horizontal(CAGE, 64))];
    let lib = library(vec![spawner, lone]);
    let (mut cells, mut matched) = setup(
        vec![((0, 0, 0), vec![horizontal(CAGE, 64), side(CAGE, (1, 0, 0))]), (up, vec![horizontal(CAGE, 0)])],
        &lib,
    );
    assert_eq!(credit(&lib, &mut cells, &mut matched), 1);
    assert!(!cells.contains_key(&up));
    assert_eq!(cells[&(0, 0, 0)].keys.len(), 2);
}

#[test]
fn credit_ignores_unmatched_sources() {
    let lib = lib();
    let (mut cells, mut matched) =
        setup(vec![((0, 0, 0), vec![side("minecraft:block/unknown", (0, 1, 0))]), (EAST, stone_with_board())], &lib);
    assert_eq!(credit(&lib, &mut cells, &mut matched), 0);
    assert_eq!(matched[&EAST], None);
}

#[test]
fn strip_foreign_drops_faces_an_observed_neighbour_may_have_drawn() {
    let lib = lib();
    let (mut cells, mut matched) =
        setup(vec![((0, 0, 0), vec![side("minecraft:block/unknown", (0, 1, 0))]), (EAST, stone_with_board())], &lib);
    assert_eq!(strip_foreign(&lib, &mut cells, &mut matched), 1);
    assert_eq!(matched[&EAST], Some((id(&lib, "minecraft:stone"), How::Exact)));
    assert_eq!(matched[&(0, 0, 0)], None);
}

#[test]
fn strip_foreign_drops_a_duplicate_before_the_blocks_own_face() {
    const FIRE: &str = "minecraft:block/fire_0";
    let east_plane = side(FIRE, (1, 0, 0));
    let mut fire = entry("minecraft:fire", vec![horizontal(FIRE, 32), east_plane]);
    // the fire to the east draws its west plane on this cell's east boundary too
    fire.overhang = vec![((-1, 0, 0), east_plane)];
    let lib = library(vec![fire, entry("minecraft:stone", cube(STONE))]);
    let (mut cells, mut matched) = setup(
        vec![((0, 0, 0), vec![horizontal(FIRE, 32), east_plane, east_plane]), (EAST, vec![side(STONE, (0, 1, 0))])],
        &lib,
    );
    assert_eq!(matched[&(0, 0, 0)], None, "the duplicate plane is an extra face");
    assert_eq!(strip_foreign(&lib, &mut cells, &mut matched), 1);
    assert_eq!(cells[&(0, 0, 0)].keys.len(), 2, "one copy of its own plane stays");
    assert_eq!(matched[&(0, 0, 0)], Some((id(&lib, "minecraft:fire"), How::Exact)));
}

#[test]
fn strip_foreign_needs_the_source_cell_observed() {
    let lib = lib();
    let (mut cells, mut matched) = setup(vec![(EAST, stone_with_board())], &lib);
    assert_eq!(strip_foreign(&lib, &mut cells, &mut matched), 0);
    assert_eq!(matched[&EAST], None);
    assert_eq!(cells[&EAST].keys.len(), 7);
}
