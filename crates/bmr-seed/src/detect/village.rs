//! Villages: the start piece (town centre) is rotated about its template origin, which sits on the chunk corner.
//! Every town centre but one rare zombie variant has a bell, so each bell gives origin = bell − rotate(local) per
//! (template, bell, rotation); only chunk-aligned origins are kept, and the template must then match around them.
//! House bells (plains meeting points 4/5) and player bells fail the match.

use super::cluster::Cluster;
use super::template::{Hypothesis, ROTATIONS, Transform, templates};

pub fn hypotheses(bells: &Cluster) -> Vec<Hypothesis> {
    let centres = templates().iter().filter(|t| t.name.contains("/town_centers/"));
    let mut out = Vec::new();
    for t in centres {
        for local in t.find("bell") {
            for m in ROTATIONS {
                let r = Transform::rotated(m, [0, 0], [0, 0, 0]).apply(local);
                for &b in &bells.points {
                    let origin = [b[0] - r[0], b[1] - r[1], b[2] - r[2]];
                    if origin[0] % 16 == 0 && origin[2] % 16 == 0 {
                        let at = Transform::rotated(m, [0, 0], origin);
                        out.push(Hypothesis { template: t, at, start: [origin[0] / 16, origin[2] / 16] });
                    }
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
    use super::super::template::template;
    use super::*;

    fn stamp(name: &str, at: Transform) -> HashMap<[i32; 3], String> {
        template(name).blocks.iter().map(|(p, n)| (at.apply(*p), n.clone())).collect()
    }

    fn bells_in(world: &HashMap<[i32; 3], String>) -> Cluster {
        let points: Vec<[i32; 3]> = world.iter().filter(|(_, n)| *n == "bell").map(|(p, _)| *p).collect();
        Cluster { bbox: Bbox::at(points[0]), points }
    }

    #[test]
    fn every_rotation_recovers_the_start_and_scores_full() {
        for name in
            ["village/plains/town_centers/plains_meeting_point_3", "village/desert/town_centers/desert_meeting_point_1"]
        {
            for m in ROTATIONS {
                let world = stamp(name, Transform::rotated(m, [0, 0], [-48, 70, 160]));
                let hits: Vec<_> =
                    hypotheses(&bells_in(&world)).into_iter().filter(|h| h.score(&world) > 0.99).collect();
                assert!(!hits.is_empty(), "{name} {m:?}");
                assert!(hits.iter().all(|h| h.start == [-3, 10]), "{name} {m:?}");
            }
        }
    }

    #[test]
    fn a_house_bell_matches_no_town_centre() {
        // plains_meeting_point_4 (a house) has its bell at local (7, 6); put it where a centre would align
        let mut world = HashMap::new();
        world.insert([32 + 7, 64, 16 + 6], "bell".to_string());
        let best = hypotheses(&bells_in(&world)).iter().map(|h| h.score(&world)).fold(0.0, f64::max);
        assert!(best < 0.05, "{best}");
    }
}
