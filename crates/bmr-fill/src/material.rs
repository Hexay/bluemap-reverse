//! Material for solid gaps (shape is evidence-based, material is a prior). Two passes:
//! 1. Strong rules: bedrock at the floor; depth-based layering below a natural terrain block observed
//!    directly above the gap (grass → dirt ×3 → stone/deepslate).
//! 2. Gaps under builds: copy the nearest column resolved in pass 1 at the same heights (grass under a
//!    wool floor), `copy.rs`.
//!
//! Below the layers, both fall back to the map's typical block at that height (`typical.rs`), else by
//! height (deepslate at y ≤ 0, stone above).

use rustc_hash::FxHashMap;

use bmr_world::{BlockRegistry, BlockState};

use crate::columns::Column;
use crate::copy::{Sources, copy_nearest};
use crate::profile::{Kind, Profile};
use crate::typical::Typical;

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

/// `full_cubes`: observed full-cube blocks (pass 2 copies these as well as pass-1 results); `rock_at`: the
/// column's biome rock (`biome_rock`).
pub fn solid_segments<'a>(
    gaps: &[SolidGap],
    profile: &Profile,
    registry: &BlockRegistry,
    full_cubes: impl IntoIterator<Item = ((i32, i32, i32), &'a BlockState)>,
    rock_at: impl Fn(Column) -> Option<&'static str>,
) -> Vec<Run> {
    let named = |name: &str| {
        let props = registry.get(name).map(|b| b.default.clone()).unwrap_or_default();
        BlockState::new(name.to_owned(), props)
    };
    let mut layered = Vec::new();
    let mut deferred = Vec::new();
    for g in gaps {
        // end bedrock is always built (pillar tops, exit portal): copy the walls instead
        let natural = |s: &&BlockState| profile.bedrock_floor() || s.name != "minecraft:bedrock";
        match g.above.filter(natural).and_then(|s| surface_kind(&s.name, profile).map(|k| (s, k))) {
            Some((above, kind)) => layered.push((g, above, layers(kind, g, profile, rock_at(g.column)))),
            None => deferred.push(g),
        }
    }
    let evidence = layered.iter().flat_map(|(g, above, l)| {
        let fills = l.runs.iter().flat_map(|(lo, hi, name)| (*lo..=*hi).map(move |y| (y, name.as_str())));
        fills.chain([(g.yhi + 1, above.name.as_str())])
    });
    let surfaces: Vec<i32> = layered.iter().map(|(g, _, _)| g.yhi + 1).collect();
    let typical = Typical::learn(&surfaces, evidence);

    let mut out = Vec::new();
    let mut resolved: FxHashMap<Column, Vec<(i32, i32, BlockState)>> = FxHashMap::default();
    for (g, _, l) in &layered {
        let rest = by_height_runs(g.ylo, l.rest_hi, profile, l.base.as_deref(), &typical, rock_at(g.column));
        for (ylo, yhi, name) in l.runs.iter().cloned().chain(rest) {
            let state = named(&name);
            resolved.entry(g.column).or_default().push((ylo, yhi, state.clone()));
            out.push(Run { column: g.column, ylo, yhi, state });
        }
    }
    let sources = Sources::new(&resolved, full_cubes);
    for g in deferred {
        out.extend(copy_nearest(g, &sources, |y| default_at(y, profile, &typical, rock_at(g.column)), &named));
    }
    out
}

/// What layering a surface block implies below it; `None` for builds and anything non-terrain.
fn surface_kind<'n>(name: &'n str, profile: &Profile) -> Option<&'n str> {
    let s = name.strip_prefix("minecraft:")?;
    // end spikes are obsidian from the world floor up; obsidian elsewhere in the end is rare
    if profile.kind == Kind::End && s == "obsidian" {
        return Some(s);
    }
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

/// What a terrain surface implies for the gap below it: explicit layers (evidence for `Typical`), then
/// [gap.ylo, rest_hi] left to the by-height fill, continuing `base` if the surface block runs deep.
struct Layers {
    runs: Vec<(i32, i32, String)>,
    rest_hi: i32,
    base: Option<String>,
}

fn layers(kind: &str, g: &SolidGap, profile: &Profile, rock: Option<&str>) -> Layers {
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
    let continues = CONTINUING.contains(&kind) || kind == "obsidian";
    let base = continues.then(|| rock.map_or_else(|| format!("minecraft:{kind}"), str::to_owned));
    Layers { runs, rest_hi: y, base }
}

/// Rock that fills a biome's hidden volume instead of the dimension's default (measured on the nether
/// fixture: basalt is 58–71% of hidden deltas cells at every depth, netherrack ~20%).
pub fn biome_rock(biome: &str) -> Option<&'static str> {
    (biome == "minecraft:basalt_deltas").then_some("minecraft:basalt")
}

/// Unseen block at `y` with no other evidence: the map's typical block, else the biome's rock (not over
/// the dimension's bedrock), else the profile's default.
pub fn default_at<'t>(y: i32, profile: &Profile, typical: &'t Typical, rock: Option<&'static str>) -> &'t str {
    typical.at(y).unwrap_or_else(|| match profile.default_block(y) {
        "minecraft:bedrock" => "minecraft:bedrock",
        d => rock.unwrap_or(d),
    })
}

/// Default fill for [ylo, yhi], one cell at a time merged into runs: `base` if given (not over the
/// dimension's bedrock), else `default_at`.
fn by_height_runs(
    ylo: i32,
    yhi: i32,
    profile: &Profile,
    base: Option<&str>,
    typical: &Typical,
    rock: Option<&'static str>,
) -> Vec<(i32, i32, String)> {
    let mut runs: Vec<(i32, i32, String)> = Vec::new();
    for y in (ylo..=yhi).rev() {
        let default = profile.default_block(y);
        let name = match base {
            Some(b) if default != "minecraft:bedrock" => b.to_owned(),
            _ => default_at(y, profile, typical, rock).to_owned(),
        };
        match runs.last_mut() {
            Some(r) if r.2 == name => r.0 = y,
            _ => runs.push((y, y, name)),
        }
    }
    runs
}
