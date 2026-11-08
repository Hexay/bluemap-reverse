//! Structures found by template match: marker clusters propose placements (template, transform, start chunk), and
//! a start is kept only when the placed template's blocks are actually there. For structures whose bbox alone is
//! ambiguous (village houses, snowy houses, spruce builds) or whose start piece is a small part of a big layout.

use std::collections::HashSet;

use anyhow::Result;
use bmr_world::World;

use super::blocks::{BlockSource, Blocks};
use super::cluster::{Bbox, Cluster};
use super::rules::{SEA_LEVEL, names};
use super::template::Hypothesis;
use super::{Detection, huts, mansion, village};

/// Fraction of a template's signature blocks that must match. On `fixtures/structures` true placements score
/// 0.82–1.0 in the reconstruction (0.96 even for the outpost watchtower's interior), the best wrong one 0.40
/// (a fountain over village paths) and a 1-block height error 0.32.
const MIN_SCORE: f64 = 0.6;
/// Placements within this of the best keep their start too (a near-tie is ambiguous, not decided by noise).
const TIE: f64 = 0.1;

pub struct TemplateRule {
    pub set: &'static str,
    pub markers: fn() -> Vec<String>,
    pub link: i32,
    pub min_points: usize,
    pub accepts: fn(&Bbox) -> bool,
    pub hypotheses: fn(&Cluster) -> Vec<Hypothesis>,
}

pub static TEMPLATE_RULES: &[TemplateRule] = &[
    TemplateRule {
        set: "villages",
        // bells group into one village
        markers: || names(&["bell"]),
        link: 64,
        min_points: 1,
        accepts: |_| true,
        hypotheses: village::hypotheses,
    },
    TemplateRule {
        set: "igloos",
        // the 7×8 snow dome (natural snow fields cluster far larger)
        markers: || names(&["snow_block"]),
        link: 1,
        min_points: 30,
        accepts: |b| matches!(b.span_xz(), (6..=8, 6..=8)),
        hypotheses: huts::igloo,
    },
    TemplateRule {
        set: "swamp_huts",
        // 7×9 roof and walls on oak-log stilts (shipwrecks are longer and sunk)
        markers: || names(&["spruce_planks", "spruce_stairs"]),
        link: 2,
        min_points: 25,
        accepts: |b| {
            let (x, z) = b.span_xz();
            (6..=8).contains(&x.min(z)) && (8..=10).contains(&x.max(z)) && b.max[1] >= SEA_LEVEL
        },
        hypotheses: huts::swamp_hut,
    },
    TemplateRule {
        set: "woodland_mansions",
        // not dark oak logs: dark forests are full of them
        markers: || names(&["dark_oak_planks", "birch_planks"]),
        link: 4,
        min_points: 200,
        accepts: |b| b.span_xz().0.min(b.span_xz().1) >= 24,
        hypotheses: mansion::hypotheses,
    },
];

/// One detection per accepted cluster; its chunks are the starts of the best-matching placements.
pub fn detect(world: &World, clusters: Vec<(&'static TemplateRule, Cluster)>) -> Result<Vec<Detection>> {
    let proposed: Vec<Vec<Hypothesis>> = clusters.iter().map(|(rule, c)| (rule.hypotheses)(c)).collect();
    let chunks: HashSet<[i32; 2]> = proposed.iter().flatten().flat_map(Hypothesis::chunks).collect();
    let blocks = Blocks::load(world, &chunks)?;
    Ok(clusters.iter().zip(&proposed).map(|((rule, c), hs)| judge(rule.set, c, hs, &blocks)).collect())
}

fn judge(set: &'static str, c: &Cluster, hs: &[Hypothesis], world: &dyn BlockSource) -> Detection {
    let scored: Vec<(f64, [i32; 2])> = hs.iter().map(|h| (h.score(world), h.start)).collect();
    let best = scored.iter().map(|s| s.0).fold(0.0, f64::max);
    let mut chunks: Vec<[i32; 2]> =
        scored.iter().filter(|(s, _)| *s >= MIN_SCORE && *s >= best - TIE).map(|s| s.1).collect();
    chunks.sort_unstable();
    chunks.dedup();
    Detection { set, min: c.bbox.min, max: c.bbox.max, blocks: c.points.len(), chunks, score: Some(best) }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::template::{ROTATIONS, Transform, template};
    use super::*;

    #[test]
    fn a_partial_match_or_a_tie_is_judged_conservatively() {
        let t = template("village/plains/town_centers/plains_meeting_point_1");
        let at = |x: i32| Transform::rotated(ROTATIONS[0], [0, 0], [x, 64, 0]);
        let full: HashMap<[i32; 3], String> = t.blocks.iter().map(|(p, n)| (at(0).apply(*p), n.clone())).collect();
        let c = Cluster { bbox: Bbox::at([0, 64, 0]), points: vec![[0, 64, 0]] };
        let h = |x: i32| Hypothesis { template: t, at: at(x), start: [x / 16, 0] };

        // the true placement wins; a shifted one scoring low is dropped
        assert_eq!(judge("villages", &c, &[h(0), h(16)], &full).chunks, vec![[0, 0]]);
        // half the blocks gone: below MIN_SCORE, no observation
        let half: HashMap<_, _> = full.iter().filter(|(p, _)| p[0] % 2 == 0).map(|(p, n)| (*p, n.clone())).collect();
        assert!(judge("villages", &c, &[h(0)], &half).chunks.is_empty());
        // two equally good placements: both kept
        let mut twice = full.clone();
        twice.extend(t.blocks.iter().map(|(p, n)| (at(32).apply(*p), n.clone())));
        assert_eq!(judge("villages", &c, &[h(0), h(32)], &twice).chunks, vec![[0, 0], [2, 0]]);
    }
}
