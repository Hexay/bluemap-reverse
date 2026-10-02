//! Villages: the start piece (town centre) is rotated about its template origin, which sits on the chunk corner.
//! Almost every town centre has a bell at a known local position, so each bell gives origin = bell −
//! rotate(local) for every (template, rotation); only chunk-aligned origins survive.

use super::cluster::Bbox;

/// Local (x, z) of the bells in 26.3's town centres (`structure/village/*/town_centers`, zombie variants too).
const BELLS: [(i32, i32); 18] = [
    (10, 4),
    (3, 1),
    (6, 2),
    (4, 2),
    (3, 7),
    (5, 10),
    (5, 1),
    (5, 9),
    (6, 6),
    (1, 1),
    (9, 9),
    (4, 5),
    (4, 4),
    (1, 7),
    (9, 5),
    (1, 4),
    (3, 3),
    (9, 3),
];

/// Origin estimates for a bell at `bell.min`: rotations NONE (x,z), CW90 (−z,x), CW180 (−x,−z), CCW90 (z,−x).
pub fn corners(bell: &Bbox) -> Vec<(i32, i32)> {
    let (bx, bz) = (bell.min[0], bell.min[2]);
    BELLS.iter().flat_map(|&(x, z)| [(bx - x, bz - z), (bx + z, bz - x), (bx + x, bz + z), (bx - z, bz + x)]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rotation_recovers_the_origin() {
        let origin = (32, 48);
        // plains_meeting_point_1's bell at local (3, 7), rotated about the origin
        for (dx, dz) in [(3, 7), (-7, 3), (-3, -7), (7, -3)] {
            let bell = Bbox::at([origin.0 + dx, 70, origin.1 + dz]);
            assert!(corners(&bell).contains(&origin), "{dx},{dz}");
        }
    }
}
