//! Igloos and swamp huts: one small piece whose footprint pins the start chunk (structure_start_rules.md §1-2);
//! each rotation's chunk-aligned footprint becomes a hypothesis, and the full shape must then match.

use std::sync::OnceLock;

use super::cluster::Cluster;
use super::template::{Hypothesis, ROTATIONS, Template, Transform, template};

/// Chunk corners within `tol` blocks (per axis) of `v`.
fn corners_near(v: [i32; 2], tol: i32) -> Vec<[i32; 2]> {
    let axis = |v: i32| (v - tol..=v + tol).filter(|c| c % 16 == 0).collect::<Vec<_>>();
    axis(v[0]).into_iter().flat_map(|x| axis(v[1]).into_iter().map(move |z| [x, z])).collect()
}

/// Placements `place(corner, y)` whose extent of `blocks` (the cluster's markers) starts within one block of the
/// cluster's, for y = cluster bottom − their lowest local y ± 1.
fn footprint_hypotheses(
    c: &Cluster,
    t: &'static Template,
    blocks: &[&str],
    places: &[fn([i32; 3]) -> Transform],
) -> Vec<Hypothesis> {
    let local: Vec<[i32; 3]> = blocks.iter().flat_map(|b| t.find(b)).collect();
    let y0 = c.bbox.min[1] - local.iter().map(|p| p[1]).min().unwrap_or(0);
    let mut out = Vec::new();
    for place in places {
        let at0 = place([0, 0, 0]);
        let min = |i: usize| local.iter().map(|&p| at0.apply(p)[i]).min().unwrap_or(0);
        for [x, z] in corners_near([c.bbox.min[0] - min(0), c.bbox.min[2] - min(2)], 1) {
            for dy in -1..=1 {
                out.push(Hypothesis { template: t, at: place([x, y0 + dy, z]), start: [x / 16, z / 16] });
            }
        }
    }
    out
}

/// igloo/top: template origin on the chunk corner, rotated about pivot (3, 5).
pub fn igloo(c: &Cluster) -> Vec<Hypothesis> {
    let places: [fn([i32; 3]) -> Transform; 4] = [
        |p| Transform::rotated(ROTATIONS[0], [3, 5], p),
        |p| Transform::rotated(ROTATIONS[1], [3, 5], p),
        |p| Transform::rotated(ROTATIONS[2], [3, 5], p),
        |p| Transform::rotated(ROTATIONS[3], [3, 5], p),
    ];
    footprint_hypotheses(c, template("igloo/top"), &["snow_block"], &places)
}

/// SwampHutPiece.postProcess as local boxes (x0, y0, z0, x1, y1, z1); the 7×9 box's min corner is the chunk corner.
const HUT: &[(&str, [i32; 6])] = &[
    ("spruce_planks", [1, 1, 1, 5, 1, 7]),
    ("spruce_planks", [1, 4, 2, 5, 4, 7]),
    ("spruce_planks", [2, 1, 0, 4, 1, 0]),
    ("spruce_planks", [2, 2, 2, 3, 3, 2]),
    ("spruce_planks", [1, 2, 3, 1, 3, 6]),
    ("spruce_planks", [5, 2, 3, 5, 3, 6]),
    ("spruce_planks", [2, 2, 7, 4, 3, 7]),
    ("oak_log", [1, 0, 2, 1, 3, 2]),
    ("oak_log", [5, 0, 2, 5, 3, 2]),
    ("oak_log", [1, 0, 7, 1, 3, 7]),
    ("oak_log", [5, 0, 7, 5, 3, 7]),
    ("spruce_stairs", [0, 4, 1, 6, 4, 1]),
    ("spruce_stairs", [0, 4, 2, 0, 4, 7]),
    ("spruce_stairs", [6, 4, 2, 6, 4, 7]),
    ("spruce_stairs", [0, 4, 8, 6, 4, 8]),
];

/// Later boxes overwrite earlier ones; the window gaps, fences and furnishings are left out.
fn hut() -> &'static Template {
    static HUT_TEMPLATE: OnceLock<Template> = OnceLock::new();
    HUT_TEMPLATE.get_or_init(|| {
        let mut cells = std::collections::BTreeMap::new();
        for &(name, [x0, y0, z0, x1, y1, z1]) in HUT {
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        cells.insert([x, y, z], name.to_string());
                    }
                }
            }
        }
        for gap in [[2, 3, 2], [3, 3, 7], [1, 3, 4], [5, 3, 4], [5, 3, 5], [1, 3, 5]] {
            cells.remove(&gap);
        }
        Template::new("swamp_hut", cells)
    })
}

/// Old-style piece orientations (StructurePiece.getWorldX/Z) for the 7-wide, 9-deep box.
pub fn swamp_hut(c: &Cluster) -> Vec<Hypothesis> {
    let places: [fn([i32; 3]) -> Transform; 4] = [
        |[x, y, z]| Transform { m: [1, 0, 0, -1], t: [x, y, z + 8] },
        |[x, y, z]| Transform { m: [1, 0, 0, 1], t: [x, y, z] },
        |[x, y, z]| Transform { m: [0, -1, 1, 0], t: [x + 8, y, z] },
        |[x, y, z]| Transform { m: [0, 1, 1, 0], t: [x, y, z] },
    ];
    footprint_hypotheses(c, hut(), &["spruce_planks", "spruce_stairs"], &places)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::cluster::Bbox;
    use super::*;

    fn cluster_of(world: &HashMap<[i32; 3], String>, names: &[&str]) -> Cluster {
        let points: Vec<[i32; 3]> =
            world.iter().filter(|(_, n)| names.contains(&n.as_str())).map(|(p, _)| *p).collect();
        let mut bbox = Bbox::at(points[0]);
        for p in &points {
            bbox.min = std::array::from_fn(|i| bbox.min[i].min(p[i]));
            bbox.max = std::array::from_fn(|i| bbox.max[i].max(p[i]));
        }
        Cluster { bbox, points }
    }

    fn best(hs: Vec<Hypothesis>, world: &HashMap<[i32; 3], String>) -> Vec<[i32; 2]> {
        let mut starts: Vec<[i32; 2]> = hs.into_iter().filter(|h| h.score(world) > 0.99).map(|h| h.start).collect();
        starts.dedup();
        starts
    }

    #[test]
    fn igloo_every_rotation() {
        for (i, m) in ROTATIONS.into_iter().enumerate() {
            let place = Transform::rotated(m, [3, 5], [160, 70, -32]);
            let world: HashMap<_, _> =
                template("igloo/top").blocks.iter().map(|(p, n)| (place.apply(*p), n.clone())).collect();
            assert_eq!(best(igloo(&cluster_of(&world, &["snow_block"])), &world), vec![[10, -2]], "rotation {i}");
        }
    }

    #[test]
    fn swamp_hut_every_orientation() {
        let places: [Transform; 4] = [
            Transform { m: [1, 0, 0, -1], t: [-64, 64, 48 + 8] },
            Transform { m: [1, 0, 0, 1], t: [-64, 64, 48] },
            Transform { m: [0, -1, 1, 0], t: [-64 + 8, 64, 48] },
            Transform { m: [0, 1, 1, 0], t: [-64, 64, 48] },
        ];
        for at in places {
            let world: HashMap<_, _> = hut().blocks.iter().map(|(p, n)| (at.apply(*p), n.clone())).collect();
            let c = cluster_of(&world, &["spruce_planks", "spruce_stairs"]);
            assert_eq!(c.bbox.span_xz().0.min(c.bbox.span_xz().1), 7);
            assert_eq!(best(swamp_hut(&c), &world), vec![[-4, 3]], "{at:?}");
        }
    }
}
