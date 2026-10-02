use super::*;
use crate::test_util::world_side;

/// Top face of the block at the origin with the given AO per vertex (in `world_side`'s vertex order).
fn top(ao: [u8; 4]) -> WorldFace {
    let mut f = world_side((0, 0, 0), (0, 1, 0), "minecraft:block/stone", [255; 3], 0);
    f.ao = ao;
    f
}

#[test]
fn corners_look_into_the_layer_above_a_top_face() {
    let face = AoFace::of(&top([255; 4])).expect("full boundary face");
    let corners: Vec<Corner> = face.corners().collect();
    assert_eq!(corners.len(), 4);
    let c = corners.iter().find(|c| c.cells[2] == (1, 1, 1)).expect("diagonal +x+z");
    assert_eq!(c.cells[..2], [(1, 1, 0), (0, 1, 1)]);
    assert!(corners.iter().all(|c| c.cells.iter().all(|cell| cell.1 == 1)));
}

#[test]
fn ao_values_map_to_occluder_counts_per_vertex() {
    let f = top([255, 191, 127, 63]);
    let face = AoFace::of(&f).unwrap();
    // each vertex's count lands on the corner its position points to
    for (v, ao) in f.verts.iter().zip(f.ao) {
        let du = if v[0] > 0.5 { 1 } else { -1 };
        let dw = if v[2] > 0.5 { 1 } else { -1 };
        let c = face.corners().find(|c| c.cells[2] == (du, 1, dw)).unwrap();
        assert_eq!(c.occluders, (255 - ao) / 64);
    }
}

#[test]
fn off_scale_ao_and_partial_faces_are_ignored() {
    assert!(AoFace::of(&top([255, 255, 200, 255])).is_none());
    let mut slab = top([255; 4]);
    slab.verts.iter_mut().for_each(|v| v[1] = 0.5);
    assert!(AoFace::of(&slab).is_none());
}

fn corner(occluders: u8) -> Corner {
    Corner { cells: [(1, 0, 0), (0, 0, 1), (1, 0, 1)], occluders }
}

#[test]
fn known_cells_decide_the_rest() {
    let known = |c: Cell| if c == (1, 0, 0) { Known::Occluder } else { Known::Unseen };
    assert_eq!(decide(&corner(1), known), [((0, 0, 1), false), ((1, 0, 1), false)]);
    assert_eq!(decide(&corner(3), known), [((0, 0, 1), true), ((1, 0, 1), true)]);
    assert!(decide(&corner(2), known).is_empty(), "one of two: undecided");
}

#[test]
fn an_unsure_cell_or_a_contradiction_decides_nothing() {
    let unsure = |c: Cell| if c == (1, 0, 0) { Known::Unsure } else { Known::Unseen };
    assert!(decide(&corner(0), unsure).is_empty());
    let all_occluding = |_| Known::Occluder;
    assert!(decide(&corner(1), all_occluding).is_empty());
}
