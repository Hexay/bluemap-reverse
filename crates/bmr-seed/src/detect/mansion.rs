//! Woodland mansions: the layout is random but the entrance piece sits at a fixed offset from the start chunk per
//! rotation (structure_start_rules.md §4), so every chunk corner whose entrance would lie inside the cluster is tried
//! with each rotation and a few heights.

use super::cluster::Cluster;
use super::template::{Hypothesis, ROTATIONS, Transform, template};

/// Entrance template position relative to (X0, Z0) for NONE, CW90, CW180, CCW90: (X0+7, Z0+7) + 9·rotate(WEST).
const ENTRANCE: [[i32; 2]; 4] = [[-2, 7], [7, -2], [16, 7], [7, 16]];

pub fn hypotheses(c: &Cluster) -> Vec<Hypothesis> {
    let t = template("woodland_mansion/entrance");
    let (lo, hi) = (c.bbox.min, c.bbox.max);
    let mut out = Vec::new();
    for (m, [ox, oz]) in ROTATIONS.into_iter().zip(ENTRANCE) {
        let at0 = Transform::rotated(m, [0, 0], [ox, 0, oz]);
        let ext = |i: usize| {
            let v = t.blocks.iter().map(|(p, _)| at0.apply(*p)[i]);
            (v.clone().min().unwrap_or(0), v.max().unwrap_or(0))
        };
        let ((x0, x1), (z0, z1)) = (ext(0), ext(2));
        // corners whose entrance footprint lies inside the cluster bbox (± 2 for eroded edges)
        let xs = (lo[0] - 2 - x0..=hi[0] + 2 - x1).filter(|x| x % 16 == 0);
        for x in xs {
            for z in (lo[2] - 2 - z0..=hi[2] + 2 - z1).filter(|z| z % 16 == 0) {
                for dy in -3..=3 {
                    let at = Transform::rotated(m, [0, 0], [x + ox, lo[1] - 1 + dy, z + oz]);
                    out.push(Hypothesis { template: t, at, start: [x / 16, z / 16] });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::cluster::Bbox;
    use super::*;

    #[test]
    fn entrance_pins_the_start_in_every_rotation() {
        let t = template("woodland_mansion/entrance");
        for (m, [ox, oz]) in ROTATIONS.into_iter().zip(ENTRANCE) {
            let at = Transform::rotated(m, [0, 0], [320 + ox, 70, -160 + oz]);
            let world: HashMap<[i32; 3], String> = t.blocks.iter().map(|(p, n)| (at.apply(*p), n.clone())).collect();
            // a mansion body around the entrance: the cluster extends well past it
            let points: Vec<[i32; 3]> = world.keys().copied().collect();
            let mut bbox = Bbox::at(points[0]);
            bbox.min = [280, 69, -200];
            bbox.max = [360, 90, -120];
            let starts: Vec<[i32; 2]> = hypotheses(&Cluster { bbox, points })
                .into_iter()
                .filter(|h| h.score(&world) > 0.99)
                .map(|h| h.start)
                .collect();
            assert!(!starts.is_empty() && starts.iter().all(|s| *s == [20, -10]), "{m:?}: {starts:?}");
        }
    }
}
