//! Structures in a (reconstructed) world → observations for the cracker: marker-block clusters per structure
//! type, filtered by shape, turned into candidate start chunks (rules.rs).

mod cluster;
mod evaluate;
mod portal;
mod rules;
mod scan;
mod village;

pub use evaluate::{SetScore, evaluate};

use anyhow::Result;
use bmr_world::World;
use serde::Serialize;

pub use cluster::Bbox;

use crate::observation::Observation;
use rules::RULES;

#[derive(Clone, Debug, Serialize)]
pub struct Detection {
    pub set: &'static str,
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub blocks: usize,
    /// candidate start chunks; empty = the shape fits but no corner estimate lands on a chunk corner
    pub chunks: Vec<[i32; 2]>,
}

impl Detection {
    pub fn observation(&self) -> Observation {
        Observation { set: self.set.to_string(), chunks: self.chunks.clone() }
    }
}

pub struct Scan {
    pub detections: Vec<Detection>,
    /// chunks present in the world (what could have been seen)
    pub chunks: Vec<[i32; 2]>,
}

pub fn detect(world: &World) -> Result<Scan> {
    let markers: Vec<Vec<String>> = RULES.iter().map(|r| (r.markers)()).collect();
    let marker_refs: Vec<Vec<&str>> = markers.iter().map(|m| m.iter().map(String::as_str).collect()).collect();
    let slices: Vec<&[&str]> = marker_refs.iter().map(Vec::as_slice).collect();
    let scan = scan::marker_positions(world, &slices)?;

    let mut out = Vec::new();
    for (rule, points) in RULES.iter().zip(scan.positions) {
        let mut found: Vec<Detection> = cluster::clusters(&points, rule.link)
            .into_iter()
            .filter(|c| c.points.len() >= rule.min_points && (rule.accepts)(&c.bbox))
            .map(|c| {
                let anchors = if rule.per_point { c.points.iter().map(|&p| Bbox::at(p)).collect() } else { vec![c.bbox] };
                let estimates: Vec<(i32, i32)> = anchors.iter().flat_map(rule.corners).collect();
                Detection {
                    set: rule.set,
                    min: c.bbox.min,
                    max: c.bbox.max,
                    blocks: c.points.len(),
                    chunks: candidate_chunks(&estimates, rule.tolerance),
                }
            })
            .collect();
        if rule.set == "ocean_ruins" {
            drop_satellites(&mut found);
        }
        out.extend(found);
    }
    Ok(Scan { detections: out, chunks: scan.chunks })
}

/// Chunks whose min corner is within `tolerance` blocks (per axis) of an estimate.
fn candidate_chunks(estimates: &[(i32, i32)], tolerance: i32) -> Vec<[i32; 2]> {
    let snap = |v: i32| {
        let r = v.rem_euclid(16);
        if r <= tolerance {
            Some((v - r) / 16)
        } else if 16 - r <= tolerance {
            Some((v + 16 - r) / 16)
        } else {
            None
        }
    };
    let mut out: Vec<[i32; 2]> =
        estimates.iter().filter_map(|&(x, z)| Some([snap(x)?, snap(z)?])).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// A big ocean ruin (16×16) spawns 4–8 small satellites around it; only the big one sits on the start chunk.
/// A lone small ruin has none, so small ruins near a big one or near each other are satellites (their big ruin
/// may be buried or undetected).
fn drop_satellites(ruins: &mut Vec<Detection>) {
    let big = |d: &Detection| d.max[0] - d.min[0] >= 11 && d.max[2] - d.min[2] >= 11;
    let near = |a: &Detection, b: &Detection| {
        a.min[0] <= b.max[0] + 24 && a.max[0] >= b.min[0] - 24 && a.min[2] <= b.max[2] + 24 && a.max[2] >= b.min[2] - 24
    };
    let satellite: Vec<bool> = ruins
        .iter()
        .enumerate()
        .map(|(i, d)| !big(d) && ruins.iter().enumerate().any(|(j, o)| i != j && near(d, o)))
        .collect();
    let mut i = 0;
    ruins.retain(|_| {
        i += 1;
        !satellite[i - 1]
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_to_chunk_corners() {
        assert_eq!(candidate_chunks(&[(33, -15), (40, 0)], 1), vec![[2, -1]]);
        assert_eq!(candidate_chunks(&[(31, 16)], 1), vec![[2, 1]]);
    }
}
