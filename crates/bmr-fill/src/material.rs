//! Material for hidden solid cells (shape is exact, material is a prior). Two passes:
//! 1. Strong rules: bedrock at the floor; depth-based layering below a natural terrain surface
//!    (grass → dirt ×3 → stone/deepslate).
//! 2. Remaining cells (under builds, deep interiors): majority of the nearest known full cubes at the same
//!    height (observed, or resolved in pass 1), else by height (deepslate at y ≤ 0, stone above).

use std::collections::{HashMap, HashSet, VecDeque};

use bmr_invert::face::Cell;
use bmr_world::{BlockRegistry, BlockState};

/// How far the same-height search walks through hidden cells.
const SAME_Y_RADIUS: i32 = 16;

pub struct Known<'a> {
    pub observed: &'a HashMap<Cell, &'a BlockState>,
    /// Observed cells that render as full cubes.
    pub full_cubes: &'a HashSet<Cell>,
    pub hidden: &'a HashSet<Cell>,
    pub min_y: i32,
    pub registry: &'a BlockRegistry,
}

/// Material for every hidden cell.
pub fn materials(k: &Known) -> HashMap<Cell, BlockState> {
    let mut resolved: HashMap<Cell, BlockState> =
        k.hidden.iter().filter_map(|&c| strong_rule(k, c).map(|name| (c, named(k, name)))).collect();
    let rest: Vec<Cell> = k.hidden.iter().copied().filter(|c| !resolved.contains_key(c)).collect();
    let second: Vec<(Cell, BlockState)> = rest
        .into_iter()
        .map(|c| (c, same_height(k, &resolved, c).unwrap_or_else(|| named(k, by_height(c.1).into()))))
        .collect();
    resolved.extend(second);
    resolved
}

fn named(k: &Known, name: String) -> BlockState {
    let props = k.registry.get(&name).map(|b| b.default.clone()).unwrap_or_default();
    BlockState::new(name, props)
}

fn strong_rule(k: &Known, (x, y, z): Cell) -> Option<String> {
    if y == k.min_y {
        return Some("minecraft:bedrock".into());
    }
    let mut ay = y + 1;
    while k.hidden.contains(&(x, ay, z)) {
        ay += 1;
    }
    k.observed.get(&(x, ay, z)).and_then(|s| layered(&s.name, ay - y, y))
}

/// Depth-based layering below a natural surface block; `None` if `surface` is not terrain.
fn layered(surface: &str, depth: i32, y: i32) -> Option<String> {
    let s = surface.strip_prefix("minecraft:")?;
    let under = |top: &str, n: i32| if depth <= n { format!("minecraft:{top}") } else { by_height(y).into() };
    Some(match s {
        "grass_block" | "podzol" | "mycelium" | "dirt" | "coarse_dirt" | "rooted_dirt" | "dirt_path" | "farmland" => {
            under("dirt", 3)
        }
        "sand" if depth <= 3 => "minecraft:sand".into(),
        "sand" | "sandstone" => under("sandstone", 6),
        "red_sand" if depth <= 3 => "minecraft:red_sand".into(),
        "red_sand" | "red_sandstone" => under("red_sandstone", 6),
        "gravel" => under("gravel", 2),
        "snow_block" | "powder_snow" | "snow" => under("dirt", 3),
        n if CONTINUING.contains(&n) || n.ends_with("terracotta") => format!("minecraft:{n}"),
        _ => return None,
    })
}

/// Natural blocks that usually continue downwards as themselves.
const CONTINUING: &[&str] = &[
    "stone", "deepslate", "tuff", "granite", "diorite", "andesite", "calcite", "dripstone_block", "clay", "mud",
    "netherrack", "end_stone", "blackstone", "basalt", "smooth_basalt", "packed_ice", "blue_ice", "soul_sand",
    "soul_soil", "bedrock",
];

fn by_height(y: i32) -> &'static str {
    if y <= 0 { "minecraft:deepslate" } else { "minecraft:stone" }
}

/// Majority state of the nearest known full cubes at the same height (observed full cubes or cells
/// resolved by a strong rule), reached through hidden cells.
fn same_height(k: &Known, resolved: &HashMap<Cell, BlockState>, (x, y, z): Cell) -> Option<BlockState> {
    let mut seen = HashSet::from([(x, z)]);
    let mut queue = VecDeque::from([(x, z)]);
    let mut votes: HashMap<&BlockState, u32> = HashMap::new();
    let mut found_at = None;
    while let Some((cx, cz)) = queue.pop_front() {
        let dist = (cx - x).abs() + (cz - z).abs();
        if found_at.is_some_and(|d| dist > d) || dist > SAME_Y_RADIUS {
            break;
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (cx + dx, cz + dz);
            if !seen.insert(n) {
                continue;
            }
            let cell = (n.0, y, n.1);
            let known = if k.full_cubes.contains(&cell) { Some(k.observed[&cell]) } else { resolved.get(&cell) };
            if let Some(s) = known {
                *votes.entry(s).or_default() += 1;
                found_at.get_or_insert(dist + 1);
            }
            if k.hidden.contains(&cell) {
                queue.push_back(n);
            }
        }
    }
    votes.into_iter().max_by_key(|(s, n)| (*n, std::cmp::Reverse(*s))).map(|(s, _)| s.clone())
}
