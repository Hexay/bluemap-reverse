use bmr_prbm::Group;

use super::*;

const STONE: u32 = 0;
const WATER: u32 = 1;
const LIQUID: [bool; 2] = [false, true];

/// A horizontal triangle at (x, y, z) with the cell's three corners; `up` = facing up.
struct Tri {
    at: [f32; 3],
    up: bool,
    material: u32,
    sun: i8,
    block: i8,
}

fn tri(x: f32, y: f32, z: f32, up: bool) -> Tri {
    Tri { at: [x, y, z], up, material: STONE, sun: 15, block: 0 }
}

fn dark(t: Tri) -> Tri {
    Tri { sun: 0, ..t }
}

fn tile(tris: Vec<Tri>) -> Tile {
    let mut t = Tile::default();
    for (i, tr) in tris.iter().enumerate() {
        let [x, y, z] = tr.at;
        t.position.extend([[x, y, z], [x + 1.0, y, z], [x + 1.0, y, z + 1.0]]);
        t.normal.extend([[0, if tr.up { 127 } else { -127 }, 0]; 3]);
        t.color.extend([[255; 3]; 3]);
        t.uv.extend([[0.0; 2]; 3]);
        t.ao.extend([255; 3]);
        t.sunlight.extend([tr.sun; 3]);
        t.blocklight.extend([tr.block; 3]);
        t.groups.push(Group { material: tr.material, start: 3 * i as u32, count: 3 });
    }
    t
}

#[test]
fn faces_belong_to_the_block_they_face_out_of() {
    // top of the block at y 10, bottom of the block at y 10, top of the block at y -1
    let h = DrawnHeights::of_tile(
        &tile(vec![tri(0.0, 11.0, 0.0, true), tri(0.0, 10.0, 0.0, false), tri(0.0, 0.0, 0.0, true)]),
        &LIQUID,
    );
    assert_eq!(h.faces, BTreeMap::from([(-1, 1), (10, 2)]));
    assert_eq!(h.range(), Some((-1, 10)));
}

#[test]
fn only_dark_faces_well_under_the_ocean_floor_are_cave_faces() {
    let h = DrawnHeights::of_tile(
        &tile(vec![
            tri(0.0, 21.0, 0.0, true),                           // floor of column (0, 0): block y 20
            dark(tri(0.0, 16.0, 0.0, true)),                     // y 15: BlueMap culls it, but within the slack
            dark(tri(0.0, 10.0, 0.0, false)),                    // y 10: a cave face
            tri(0.0, 9.0, 0.0, false),                           // y 9: lit
            Tri { block: 4, ..dark(tri(0.0, 8.0, 0.0, false)) }, // y 8: torch-lit
            dark(tri(3.0, 2.0, 0.0, true)),                      // y 1 in a column without a floor
        ]),
        &LIQUID,
    );
    assert_eq!(h.cave, BTreeMap::from([(10, 1)]));
}

#[test]
fn liquid_surfaces_are_not_a_floor() {
    let water = Tri { material: WATER, ..tri(0.0, 40.0, 0.0, true) };
    let h = DrawnHeights::of_tile(&tile(vec![water, dark(tri(0.0, 21.0, 0.0, true))]), &LIQUID);
    assert!(h.cave.is_empty());
}

#[test]
fn merge_adds_counts() {
    let mut a = DrawnHeights::of_tile(&tile(vec![tri(0.0, 30.0, 0.0, true), dark(tri(0.0, 5.0, 0.0, true))]), &LIQUID);
    a.merge(DrawnHeights::of_tile(&tile(vec![tri(0.0, 30.0, 0.0, true), dark(tri(0.0, 5.0, 0.0, true))]), &LIQUID));
    assert_eq!(a.faces, BTreeMap::from([(4, 2), (29, 2)]));
    assert_eq!(a.cave, BTreeMap::from([(4, 2)]));
}
