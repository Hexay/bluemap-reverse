//! Material for solid gaps (shape is evidence-based, material is a prior). Two passes:
//! 1. Strong rules: bedrock at the floor; depth-based layering below a natural terrain block observed
//!    directly above the gap (grass → dirt ×3 → stone/deepslate).
//! 2. Gaps under builds: copy the nearest column resolved in pass 1 at the same heights (grass under a
//!    wool floor), else by height (deepslate at y ≤ 0, stone above).

use rustc_hash::FxHashMap;

use bmr_world::{BlockRegistry, BlockState};

use crate::columns::{Column, ring};
use crate::profile::Profile;

/// How far pass 2 searches for a column to copy (Chebyshev radius).
const COPY_RADIUS: i32 = 16;

/// Inclusive y range of one state within a column.
#[derive(Clone)]
pub struct Run {
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
    profile: &Profile,
    registry: &BlockRegistry,
    full_cube_at: &dyn Fn((i32, i32, i32)) -> Option<BlockState>,
) -> Vec<Run> {
    let named = |name: &str| {
        let props = registry.get(name).map(|b| b.default.clone()).unwrap_or_default();
        BlockState::new(name.to_owned(), props)
    };
    let mut out = Vec::new();
    let mut resolved: FxHashMap<Column, Vec<(i32, i32, BlockState)>> = FxHashMap::default();
    let mut deferred = Vec::new();
    for g in gaps {
        // end bedrock is always built (pillar tops, exit portal): copy the walls instead
        let natural = |s: &&BlockState| profile.bedrock_floor() || s.name != "minecraft:bedrock";
        match g.above.filter(natural).and_then(|s| surface_kind(&s.name)) {
            Some(kind) => {
                for (ylo, yhi, name) in layer_profile(kind, g, profile) {
                    let state = named(&name);
                    resolved.entry(g.column).or_default().push((ylo, yhi, state.clone()));
                    out.push(Run { column: g.column, ylo, yhi, state });
                }
            }
            None => deferred.push(g),
        }
    }
    for g in deferred {
        out.extend(copy_nearest(g, &resolved, profile, &named, full_cube_at));
    }
    out
}

/// What layering a surface block implies below it; `None` for builds and anything non-terrain.
fn surface_kind(name: &str) -> Option<&str> {
    let s = name.strip_prefix("minecraft:")?;
    let terrain = matches!(
        s,
        "grass_block"
            | "podzol"
            | "mycelium"
            | "dirt"
            | "coarse_dirt"
            | "rooted_dirt"
            | "dirt_path"
            | "farmland"
            | "sand"
            | "sandstone"
            | "red_sand"
            | "red_sandstone"
            | "gravel"
            | "snow_block"
            | "powder_snow"
            | "snow"
            | "soul_sand"
            | "soul_soil"
            | "crimson_nylium"
            | "warped_nylium"
    ) || CONTINUING.contains(&s)
        || STONY.contains(&s)
        || s.ends_with("terracotta");
    terrain.then_some(s)
}

/// Blocks that usually continue downwards as themselves (layers or whole dimensions).
const CONTINUING: &[&str] =
    &["netherrack", "end_stone", "basalt", "blackstone", "smooth_basalt", "packed_ice", "blue_ice"];

/// Overworld rock: below it comes the stone/deepslate default, not more of the same (ore-like blobs).
const STONY: &[&str] = &[
    "stone",
    "deepslate",
    "tuff",
    "granite",
    "diorite",
    "andesite",
    "calcite",
    "dripstone_block",
    "clay",
    "mud",
    "bedrock",
];

/// (ylo, yhi, block name) runs for a gap below a terrain surface, top-down layering.
fn layer_profile(kind: &str, g: &SolidGap, profile: &Profile) -> Vec<(i32, i32, String)> {
    let layers: &[(&str, i32)] = match kind {
        // depths: medians under exposed surfaces on the vanilla fixture where it has enough samples
        // (sand 1 over stone, n=57k; gravel 1; exposed dirt 4; clay 1 over dirt 3); grass keeps 3 (n=31 there)
        "grass_block" | "podzol" | "mycelium" | "rooted_dirt" | "dirt_path" | "farmland" | "snow_block"
        | "powder_snow" | "snow" => &[("dirt", 3)],
        "dirt" | "coarse_dirt" => &[("dirt", 4)],
        // most sand is beach/ocean floor over stone; desert sandstone would need the biome
        "sand" => &[("sand", 1)],
        "sandstone" => &[("sandstone", 6)],
        "red_sand" => &[("red_sand", 3), ("red_sandstone", 3)],
        "red_sandstone" => &[("red_sandstone", 6)],
        "gravel" => &[("gravel", 1)],
        "clay" => &[("clay", 1), ("dirt", 3)],
        // soul sand valleys: ~4 deep over netherrack (median on the nether fixture; basalt runs deep instead)
        "soul_sand" => &[("soul_sand", 4)],
        "soul_soil" => &[("soul_soil", 4)],
        _ => &[],
    };
    let mut runs = Vec::new();
    let mut y = g.yhi;
    // the world floor is bedrock whatever lies above it
    let floor = if g.ylo == profile.min_y && profile.bedrock_floor() { g.ylo + 1 } else { g.ylo };
    for (name, depth) in layers {
        let lo = (y - depth + 1).max(floor);
        if lo <= y {
            runs.push((lo, y, format!("minecraft:{name}")));
        }
        y = lo - 1;
    }
    let base = CONTINUING.contains(&kind).then(|| format!("minecraft:{kind}"));
    runs.extend(by_height_runs(g.ylo, y, profile, base));
    runs
}

/// Default fill for [ylo, yhi], one cell at a time merged into runs: `base` if given (not over the
/// dimension's bedrock), else the profile's default block.
fn by_height_runs(ylo: i32, yhi: i32, profile: &Profile, base: Option<String>) -> Vec<(i32, i32, String)> {
    let mut runs: Vec<(i32, i32, String)> = Vec::new();
    for y in (ylo..=yhi).rev() {
        let default = profile.default_block(y);
        let name = match &base {
            Some(b) if default != "minecraft:bedrock" => b.clone(),
            _ => default.to_owned(),
        };
        match runs.last_mut() {
            Some(r) if r.2 == name => r.0 = y,
            _ => runs.push((y, y, name)),
        }
    }
    runs
}

/// Pass 2: per y, the nearest column's pass-1 result or observed full cube at that y, else by height.
fn copy_nearest(
    g: &SolidGap,
    resolved: &FxHashMap<Column, Vec<(i32, i32, BlockState)>>,
    profile: &Profile,
    named: &impl Fn(&str) -> BlockState,
    full_cube_at: &dyn Fn((i32, i32, i32)) -> Option<BlockState>,
) -> Vec<Run> {
    let (cx, cz) = g.column;
    let mut out: Vec<Run> = Vec::new();
    let mut push = |y: i32, state: BlockState| match out.last_mut() {
        Some(s) if s.state == state && s.ylo == y + 1 => s.ylo = y,
        _ => out.push(Run { column: g.column, ylo: y, yhi: y, state }),
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
        push(y, named(profile.default_block(y)));
    }
    out
}
