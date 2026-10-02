//! Tile → world faces → cells → matched states, on a hand-built 4-state library. `reverse()` itself needs a
//! mirrored `LocalMap` on disk, so this drives the same steps directly.

use std::collections::BTreeMap;

use crate::face::{Cell, DIRS, Q, faces_by_cell, signature, texture_ids, world_faces};
use crate::matcher::{How, candidates, resolve};
use crate::test_util::*;

const DIRT: &str = "minecraft:block/dirt";
const LOG: &str = "minecraft:block/oak_log";
const LOG_TOP: &str = "minecraft:block/oak_log_top";
const UNKNOWN: &str = "minecraft:block/e2e_unknown";
const NAMES: [&str; 5] = [STONE, DIRT, LOG, LOG_TOP, UNKNOWN];
const ORIGIN: [i32; 2] = [32, -16];

fn log(axis: usize) -> Vec<crate::face::FaceKey> {
    cube_with(move |d| if [d.0, d.1, d.2][axis] != 0 { LOG_TOP } else { LOG })
}

fn lib() -> crate::Library {
    let mut x = entry("minecraft:oak_log[axis=x]", log(0));
    x.default_distance = 1;
    library(vec![
        entry("minecraft:stone", cube(STONE)),
        entry("minecraft:dirt", cube(DIRT)),
        entry("minecraft:oak_log[axis=y]", log(1)),
        x,
    ])
}

/// (world cell, texture per side, sides culled by neighbours)
type Block = (Cell, fn(Cell) -> &'static str, &'static [Cell]);

fn tile(blocks: &[Block]) -> bmr_prbm::Tile {
    let mut quads = Vec::new();
    for &(cell, tex, culled) in blocks {
        for d in DIRS.into_iter().filter(|d| !culled.contains(d)) {
            let material = NAMES.iter().position(|&n| n == tex(d)).unwrap() as u32;
            let base = [cell.0 - ORIGIN[0], cell.1, cell.2 - ORIGIN[1]].map(|c| c as f32);
            let corners = side_corners(d).map(|c| std::array::from_fn(|a| base[a] + c[a] as f32 / Q));
            quads.push((material, corners, [d.0, d.1, d.2].map(|c| (c * 127) as i8)));
        }
    }
    quads.sort_by_key(|q| q.0);
    let mut b = TileBuilder::default();
    for (m, corners, normal) in quads {
        b.quad(m, corners, normal);
    }
    b.0
}

#[test]
fn hand_built_tile_resolves_to_expected_states() {
    let blocks: [Block; 5] = [
        ((33, 64, -15), |_| STONE, &[(0, 1, 0)]),
        ((33, 65, -15), |d| if d.1 != 0 { LOG_TOP } else { LOG }, &[(0, -1, 0)]),
        ((35, 64, -15), |_| DIRT, &[]),
        ((37, 64, -15), |d| if d.0 != 0 { LOG_TOP } else { LOG }, &[(1, 0, 0), (-1, 0, 0)]),
        ((39, 64, -15), |_| UNKNOWN, &[]),
    ];
    let lib = lib();
    let names: Vec<String> = NAMES.map(String::from).to_vec();
    let faces = world_faces(&tile(&blocks), ORIGIN, &texture_ids(&names));
    let got: BTreeMap<Cell, Option<(String, How)>> = faces_by_cell(&faces)
        .iter()
        .map(|(&cell, obs)| {
            let m = candidates(&lib, &signature(obs.keys.clone()));
            (cell, m.map(|c| (lib.entries[resolve(&lib, &c.ids, obs)].state.to_string(), c.how)))
        })
        .collect();

    let expected: BTreeMap<Cell, Option<(String, How)>> = [
        ((33, 64, -15), Some(("minecraft:stone", How::Partial))),
        ((33, 65, -15), Some(("minecraft:oak_log[axis=y]", How::Partial))),
        ((35, 64, -15), Some(("minecraft:dirt", How::Exact))),
        ((37, 64, -15), Some(("minecraft:oak_log[axis=x]", How::Partial))),
        ((39, 64, -15), None),
    ]
    .into_iter()
    .map(|(c, m)| (c, m.map(|(s, h)| (s.to_owned(), h))))
    .collect();
    assert_eq!(got, expected);
}
