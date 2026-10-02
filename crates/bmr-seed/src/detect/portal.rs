//! Ruined portals: the netherrack patch is centred on the template's bbox centre, and the template sits at the
//! chunk corner rotated (and maybe front-back mirrored) about its own centre, so each of the 13 templates × 4
//! rotations × 2 mirrors puts the corner at a known offset from that centre.

use super::cluster::Bbox;

/// Footprints `[x0, x1, z0, z1]` relative to (X0, Z0): rotations NONE, CW90, CW180, CCW90, then the same
/// front-back mirrored (structure_start_rules.md §7).
const FOOTPRINTS: &[[[i32; 4]; 8]] = &[
    // portal_1
    [
        [0, 5, 0, 5],
        [1, 6, 0, 5],
        [1, 6, 1, 6],
        [0, 5, 1, 6],
        [-5, 0, 0, 5],
        [1, 6, -5, 0],
        [6, 11, 1, 6],
        [0, 5, 6, 11],
    ],
    // portal_2, portal_7
    [
        [0, 8, 0, 8],
        [0, 8, 0, 8],
        [0, 8, 0, 8],
        [0, 8, 0, 8],
        [-8, 0, 0, 8],
        [0, 8, -8, 0],
        [8, 16, 0, 8],
        [0, 8, 8, 16],
    ],
    // portal_3, portal_4
    [
        [0, 7, 0, 8],
        [0, 8, 0, 7],
        [1, 8, 0, 8],
        [0, 8, 1, 8],
        [-7, 0, 0, 8],
        [0, 8, -7, 0],
        [8, 15, 0, 8],
        [0, 8, 8, 15],
    ],
    // portal_5
    [
        [0, 9, 0, 6],
        [2, 8, -2, 7],
        [1, 10, 0, 6],
        [2, 8, -1, 8],
        [-9, 0, 0, 6],
        [2, 8, -11, -2],
        [10, 19, 0, 6],
        [2, 8, 8, 17],
    ],
    // portal_6
    [
        [0, 4, 0, 6],
        [-1, 5, 1, 5],
        [0, 4, 0, 6],
        [-1, 5, 1, 5],
        [-4, 0, 0, 6],
        [-1, 5, -3, 1],
        [4, 8, 0, 6],
        [-1, 5, 5, 9],
    ],
    // portal_8
    [
        [0, 13, 0, 8],
        [3, 11, -3, 10],
        [1, 14, 0, 8],
        [3, 11, -2, 11],
        [-13, 0, 0, 8],
        [3, 11, -16, -3],
        [14, 27, 0, 8],
        [3, 11, 11, 24],
    ],
    // portal_9
    [
        [0, 9, 0, 8],
        [1, 9, -1, 8],
        [1, 10, 0, 8],
        [1, 9, 0, 9],
        [-9, 0, 0, 8],
        [1, 9, -10, -1],
        [10, 19, 0, 8],
        [1, 9, 9, 18],
    ],
    // portal_10
    [
        [0, 11, 0, 9],
        [2, 11, -1, 10],
        [1, 12, 1, 10],
        [1, 10, 0, 11],
        [-11, 0, 0, 9],
        [2, 11, -12, -1],
        [12, 23, 1, 10],
        [1, 10, 11, 22],
    ],
    // giant_portal_1, giant_portal_2
    [
        [0, 10, 0, 15],
        [-2, 13, 3, 13],
        [0, 10, 1, 16],
        [-3, 12, 3, 13],
        [-10, 0, 0, 15],
        [-2, 13, -7, 3],
        [10, 20, 1, 16],
        [-3, 12, 13, 23],
    ],
    // giant_portal_3
    [
        [0, 15, 0, 15],
        [1, 16, 0, 15],
        [1, 16, 1, 16],
        [0, 15, 1, 16],
        [-15, 0, 0, 15],
        [1, 16, -15, 0],
        [16, 31, 1, 16],
        [0, 15, 16, 31],
    ],
];

/// Corner estimates from the patch centre: centre − each footprint's centre (`BoundingBox.getCenter`).
pub fn corners(patch: &Bbox) -> Vec<(i32, i32)> {
    let (cx, cz) = patch.center_xz();
    let mut out: Vec<(i32, i32)> = FOOTPRINTS
        .iter()
        .flatten()
        .map(|&[x0, x1, z0, z1]| (cx - (x0 + (x1 - x0 + 1) / 2), cz - (z0 + (z1 - z0 + 1) / 2)))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}
