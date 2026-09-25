//! Material for solid gaps (shape is evidence-based, material is a prior). Two passes:
//! 1. Strong rules: bedrock at the floor; depth-based layering below a natural terrain block observed
//!    directly above the gap (grass → dirt ×3 → stone/deepslate).
//! 2. Gaps under builds: copy the nearest column resolved in pass 1 at the same heights (grass under a
//!    wool floor), else by height (deepslate at y ≤ 0, stone above).

use std::collections::HashMap;

use bmr_world::{BlockRegistry, BlockState};

use crate::columns::{Column, ring};

/// How far pass 2 searches for a column to copy (Chebyshev radius).
const COPY_RADIUS: i32 = 16;

/// Inclusive y range of one state within a column.
#[derive(Clone)]
pub struct Segment {
    pub column: Column,
    pub ylo: i32,
    pub yhi: i32,
    pub state: BlockState,
}

pub struct SolidGap<'a> {
    pub column: Column,
    pub ylo: i32,
    pub yhi: i32,
    /// Observed block directly above the gap.
    pub above: Option<&'a BlockState>,
}

/// `full_cube_at`: observed full-cube block at a cell (pass 2 copies these as well as pass-1 results).
pub fn solid_segments(
    gaps: &[SolidGap],
    min_y: i32,
    registry: &BlockRegistry,
    full_cube_at: &dyn Fn((i32, i32, i32)) -> Option<BlockState>,
) -> Vec<Segment> {
    let named = |name: &str| {
        let props = registry.get(name).map(|b| b.default.clone()).unwrap_or_default();
        BlockState::new(name.to_owned(), props)
    };
    let mut out = Vec::new();
    let mut resolved: HashMap<Column, Vec<(i32, i32, BlockState)>> = HashMap::new();
    let mut deferred = Vec::new();
    for g in gaps {
        match g.above.and_then(|s| surface_kind(&s.name)) {
            Some(kind) => {
                for (ylo, yhi, name) in layer_profile(kind, g, min_y) {
                    let state = named(&name);
                    resolved.entry(g.column).or_default().push((ylo, yhi, state.clone()));
                    out.push(Segment { column: g.column, ylo, yhi, state });
                }
            }
            None => deferred.push(g),
        }
    }
    for g in deferred {
        out.extend(copy_nearest(g, &resolved, min_y, &named, full_cube_at));
    }
    out
}

/// What layering a surface block implies below it; `None` for builds and anything non-terrain.
fn surface_kind(name: &str) -> Option<&str> {
    let s = name.strip_prefix("minecraft:")?;
    let terrain = matches!(
        s,
        "grass_block" | "podzol" | "mycelium" | "dirt" | "coarse_dirt" | "rooted_dirt" | "dirt_path" | "farmland"
            | "sand" | "sandstone" | "red_sand" | "red_sandstone" | "gravel" | "snow_block" | "powder_snow"
            | "snow"
    ) || CONTINUING.contains(&s)
        || STONY.contains(&s)
        || s.ends_with("terracotta");
    terrain.then_some(s)
}

/// Blocks that usually continue downwards as themselves (layers or whole dimensions).
const CONTINUING: &[&str] = &[
    "netherrack", "end_stone", "blackstone", "basalt", "smooth_basalt", "packed_ice", "blue_ice", "soul_sand",
    "soul_soil",
];

/// Overworld rock: below it comes the stone/deepslate default, not more of the same (ore-like blobs).
const STONY: &[&str] = &[
    "stone", "deepslate", "tuff", "granite", "diorite", "andesite", "calcite", "dripstone_block", "clay", "mud",
    "bedrock",
];

/// (ylo, yhi, block name) runs for a gap below a terrain surface, top-down layering.
fn layer_profile(kind: &str, g: &SolidGap, min_y: i32) -> Vec<(i32, i32, String)> {
    let layers: &[(&str, i32)] = match kind {
        "grass_block" | "podzol" | "mycelium" | "dirt" | "coarse_dirt" | "rooted_dirt" | "dirt_path" | "farmland"
        | "snow_block" | "powder_snow" | "snow" => &[("dirt", 3)],
        // most sand is beach/ocean floor over stone; desert sandstone would need the biome
        "sand" => &[("sand", 3)],
        "sandstone" => &[("sandstone", 6)],
        "red_sand" => &[("red_sand", 3), ("red_sandstone", 3)],
        "red_sandstone" => &[("red_sandstone", 6)],
        "gravel" => &[("gravel", 2)],
        _ => &[],
    };
    let mut runs = Vec::new();
    let mut y = g.yhi;
    // the world floor is bedrock whatever lies above it
    let floor = if g.ylo == min_y { g.ylo + 1 } else { g.ylo };
    for (name, depth) in layers {
        let lo = (y - depth + 1).max(floor);
        if lo <= y {
            runs.push((lo, y, format!("minecraft:{name}")));
        }
        y = lo - 1;
    }
    let base = CONTINUING.contains(&kind).then(|| format!("minecraft:{kind}"));
    runs.extend(by_height_runs(g.ylo, y, min_y, base));
    runs
}

/// Default fill for [ylo, yhi], one cell at a time merged into runs: `base` if given, else `default_block`.
fn by_height_runs(ylo: i32, yhi: i32, min_y: i32, base: Option<String>) -> Vec<(i32, i32, String)> {
    let mut runs: Vec<(i32, i32, String)> = Vec::new();
    for y in (ylo..=yhi).rev() {
        let name = match &base {
            Some(b) if y > min_y + 2 => b.clone(),
            _ => default_block(y, min_y).to_owned(),
        };
        match runs.last_mut() {
            Some(r) if r.2 == name => r.0 = y,
            _ => runs.push((y, y, name)),
        }
    }
    runs
}

/// Vanilla majority by height: bedrock floor band (100/80/60% bedrock at min_y+0/1/2), deepslate up to
/// y=3 (stone↔deepslate blends over y 0..8), stone above.
fn default_block(y: i32, min_y: i32) -> &'static str {
    if y <= min_y + 2 {
        "minecraft:bedrock"
    } else if y <= 3 {
        "minecraft:deepslate"
    } else {
        "minecraft:stone"
    }
}

/// Pass 2: per y, the nearest column's pass-1 result or observed full cube at that y, else by height.
fn copy_nearest(
    g: &SolidGap,
    resolved: &HashMap<Column, Vec<(i32, i32, BlockState)>>,
    min_y: i32,
    named: &impl Fn(&str) -> BlockState,
    full_cube_at: &dyn Fn((i32, i32, i32)) -> Option<BlockState>,
) -> Vec<Segment> {
    let (cx, cz) = g.column;
    let mut out: Vec<Segment> = Vec::new();
    let mut push = |y: i32, state: BlockState| match out.last_mut() {
        Some(s) if s.state == state && s.ylo == y + 1 => s.ylo = y,
        _ => out.push(Segment { column: g.column, ylo: y, yhi: y, state }),
    };
    'cell: for y in (g.ylo..=g.yhi).rev() {
        for r in 1..=COPY_RADIUS {
            for (dx, dz) in ring(r) {
                let col = (cx + dx, cz + dz);
                let from_pass1 = resolved
                    .get(&col)
                    .and_then(|runs| runs.iter().find(|(lo, hi, _)| (*lo..=*hi).contains(&y)))
                    .map(|(_, _, s)| s.clone());
                if let Some(s) = from_pass1.or_else(|| full_cube_at((col.0, y, col.1))) {
                    push(y, s);
                    continue 'cell;
                }
            }
        }
        push(y, named(default_block(y, min_y)));
    }
    out
}
