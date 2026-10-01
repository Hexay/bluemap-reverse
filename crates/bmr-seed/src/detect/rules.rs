//! Per-structure detection rules: marker blocks, cluster shape, and where the start chunk's corner (X0, Z0) sits
//! relative to the cluster's bbox. Placement facts: docs/research/seed-recovery/structure_start_rules.md.

use super::cluster::Bbox;
use super::{portal, village};

/// Blocks above this y are on land; ocean structures sit below it.
pub const SEA_LEVEL: i32 = 63;

pub struct Rule {
    pub set: &'static str,
    /// block names without the `minecraft:` prefix
    pub markers: fn() -> Vec<String>,
    /// cluster link distance (blocks)
    pub link: i32,
    pub min_points: usize,
    /// anchor on each marker block (as a 1-block bbox) instead of the cluster's bbox; candidates are unioned
    pub per_point: bool,
    pub accepts: fn(&Bbox) -> bool,
    /// (X0, Z0) estimates; each is snapped to a chunk corner if within `tolerance` blocks
    pub corners: fn(&Bbox) -> Vec<(i32, i32)>,
    pub tolerance: i32,
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// Worked wood only: logs also grow on trees.
fn timber() -> Vec<String> {
    let woods = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak"];
    let parts = ["planks", "stairs", "slab", "fence", "trapdoor", "door"];
    woods.iter().flat_map(|w| parts.iter().map(move |p| format!("{w}_{p}"))).collect()
}

fn spans(b: &Bbox, lo: i32, hi: i32) -> bool {
    let (x, z) = b.span_xz();
    (lo..=hi).contains(&x) && (lo..=hi).contains(&z)
}

/// Templates rotated about their origin (pivot 0) keep the origin — the chunk corner — at one bbox corner.
fn bbox_corners(b: &Bbox) -> Vec<(i32, i32)> {
    let (x0, z0, x1, z1) = (b.min[0], b.min[2], b.max[0], b.max[2]);
    vec![(x0, z0), (x1, z0), (x1, z1), (x0, z1)]
}

pub static RULES: &[Rule] = &[
    Rule {
        set: "ocean_monuments",
        markers: || {
            names(&["prismarine", "prismarine_bricks", "dark_prismarine", "sea_lantern", "prismarine_slab",
                "prismarine_brick_slab", "dark_prismarine_slab", "prismarine_stairs", "prismarine_brick_stairs"])
        },
        link: 6,
        min_points: 200,
        per_point: false,
        accepts: |b| spans(b, 30, 58),
        // 58×58 from X0-29; the far edge anchors when the near one is buried
        corners: |b| vec![(b.min[0] + 29, b.min[2] + 29), (b.max[0] - 28, b.max[2] - 28)],
        tolerance: 2,
    },
    Rule {
        set: "desert_pyramids",
        markers: || {
            names(&["orange_terracotta", "blue_terracotta", "cut_sandstone", "chiseled_sandstone", "sandstone_stairs",
                "sandstone_slab"])
        },
        link: 3,
        min_points: 40,
        per_point: false,
        accepts: |b| spans(b, 19, 21) && b.max[1] >= SEA_LEVEL,
        corners: |b| vec![(b.min[0], b.min[2])],
        tolerance: 1,
    },
    Rule {
        set: "jungle_temples",
        markers: || names(&["mossy_cobblestone", "cobblestone", "cobblestone_stairs", "chiseled_stone_bricks", "cobblestone_wall"]),
        link: 2,
        min_points: 40,
        per_point: false,
        accepts: |b| {
            let (x, z) = b.span_xz();
            let near = |v: i32, t: i32| (v - t).abs() <= 1;
            (near(x, 12) && near(z, 15)) || (near(x, 15) && near(z, 12))
        },
        corners: |b| vec![(b.min[0], b.min[2])],
        tolerance: 1,
    },
    Rule {
        set: "pillager_outposts",
        markers: || names(&["dark_oak_log", "birch_planks", "dark_oak_planks", "dark_oak_slab", "dark_oak_stairs"]),
        link: 2,
        min_points: 60,
        per_point: false,
        accepts: |b| spans(b, 13, 16),
        // 15×15 watchtower on a 16×16 base plate whose corner is the chunk corner (tower offset 0 or 1)
        corners: |b| {
            let (x0, z0, x1, z1) = (b.min[0], b.min[2], b.max[0], b.max[2]);
            vec![(x0, z0), (x0 - 1, z0), (x1, z0), (x1, z0 - 1), (x1, z1), (x1 + 1, z1), (x0, z1), (x0, z1 + 1)]
        },
        tolerance: 1,
    },
    Rule {
        set: "shipwrecks",
        markers: timber,
        link: 3,
        min_points: 25,
        // partly below sea level: beached wrecks are sunk by half their height, shore houses aren't
        per_point: false,
        accepts: |b| {
            let (x, z) = b.span_xz();
            (6..=10).contains(&x.min(z)) && (12..=30).contains(&x.max(z)) && b.min[1] < SEA_LEVEL - 1
        },
        // hull 9 wide, pivot (4,0,15): along Z X0 = minX, Z0 = minZ | maxZ-30; along X Z0 = minZ-11,
        // X0 = maxX-19 | minX+11 (each from both edges, in case one is hidden)
        corners: |b| {
            let (x0, z0, x1, z1) = (b.min[0], b.min[2], b.max[0], b.max[2]);
            if b.span_xz().0 <= b.span_xz().1 {
                vec![(x0, z0), (x0, z1 - 30), (x1 - 8, z0), (x1 - 8, z1 - 30)]
            } else {
                vec![(x1 - 19, z0 - 11), (x0 + 11, z0 - 11), (x1 - 19, z1 - 19), (x0 + 11, z1 - 19)]
            }
        },
        tolerance: 2,
    },
    Rule {
        set: "ocean_ruins",
        markers: || {
            names(&["cut_sandstone", "chiseled_sandstone", "smooth_sandstone", "sandstone_stairs", "sandstone_slab",
                "stone_bricks", "cracked_stone_bricks", "mossy_stone_bricks", "chiseled_stone_bricks",
                "stone_brick_stairs", "stone_brick_slab", "mossy_stone_brick_stairs", "mossy_stone_brick_slab",
                "suspicious_sand", "suspicious_gravel", "polished_granite"])
        },
        link: 3,
        min_points: 12,
        // underwater; small 6×7 or big 16×16 (integrity < 1 erodes edges)
        per_point: false,
        accepts: |b| b.max[1] < SEA_LEVEL - 1 && spans(b, 4, 17),
        corners: bbox_corners,
        // integrity 0.8–0.9 erodes edges
        tolerance: 2,
    },
    Rule {
        set: "swamp_huts",
        // 7×9 on oak-log stilts; roof and walls spruce (shipwrecks are longer and sunk)
        markers: || names(&["spruce_planks", "spruce_stairs", "spruce_slab", "spruce_fence"]),
        link: 2,
        min_points: 25,
        per_point: false,
        accepts: |b| {
            let (x, z) = b.span_xz();
            let near = |v: i32, t: i32| (v - t).abs() <= 1;
            ((near(x, 7) && near(z, 9)) || (near(x, 9) && near(z, 7))) && b.max[1] >= SEA_LEVEL
        },
        corners: |b| vec![(b.min[0], b.min[2])],
        tolerance: 1,
    },
    Rule {
        set: "igloos",
        // the 7×8 snow dome (natural snow fields cluster far larger)
        markers: || names(&["snow_block"]),
        link: 1,
        min_points: 30,
        per_point: false,
        accepts: |b| matches!(b.span_xz(), (6..=8, 6..=8)),
        // rotated about pivot (3,5,5): 7×8 → NONE (minX, minZ) | CW180 (minX, minZ-3);
        // 8×7 → CW90 (minX-1, minZ-2) | CCW90 (minX+2, minZ-2)
        corners: |b| {
            let (x0, z0) = (b.min[0], b.min[2]);
            if b.span_xz().0 <= b.span_xz().1 {
                vec![(x0, z0), (x0, z0 - 3)]
            } else {
                vec![(x0 - 1, z0 - 2), (x0 + 2, z0 - 2)]
            }
        },
        tolerance: 1,
    },
    Rule {
        set: "villages",
        // bells group into one village; town-centre bells pin the start chunk exactly
        markers: || names(&["bell"]),
        link: 64,
        min_points: 1,
        per_point: true,
        accepts: |_| true,
        corners: village::corners,
        tolerance: 0,
    },
    Rule {
        set: "abandoned_camp",
        // only the start piece (a tent, 8×8 or 6×8) uses wool stairs; they begin at the template origin
        markers: || names(&["white_wool_stairs"]),
        link: 2,
        min_points: 12,
        per_point: false,
        accepts: |b| spans(b, 4, 9),
        corners: bbox_corners,
        tolerance: 1,
    },
    Rule {
        set: "ruined_portals",
        // not magma: it also occurs naturally on ocean floors
        markers: || names(&["netherrack", "obsidian", "crying_obsidian", "gold_block"]),
        link: 4,
        // the corner comes from the patch centre, which a partly hidden patch skews by chunks
        min_points: 30,
        per_point: false,
        accepts: |_| true,
        corners: portal::corners,
        tolerance: 2,
    },
];
