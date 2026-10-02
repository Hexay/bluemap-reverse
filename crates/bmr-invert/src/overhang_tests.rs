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
fn strip_foreign_needs_the_source_cell_observed() {
    let lib = lib();
    let (mut cells, mut matched) = setup(vec![(EAST, stone_with_board())], &lib);
    assert_eq!(strip_foreign(&lib, &mut cells, &mut matched), 0);
    assert_eq!(matched[&EAST], None);
    assert_eq!(cells[&EAST].keys.len(), 7);
}
