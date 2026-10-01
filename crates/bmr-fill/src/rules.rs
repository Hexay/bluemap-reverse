//! Properties the render does not show but game rules determine.

use std::collections::VecDeque;

use rustc_hash::FxHashMap;

use bmr_invert::face::{Cell, DIRS};
use bmr_world::BlockState;

/// Leaves `distance` = steps to the nearest log through leaves (1..=7, vanilla #logs ≈ logs, woods,
/// stems, hyphae). Unreachable leaves (distance 7) must be persistent, or they would have decayed.
/// Returns the number of leaves changed.
pub fn leaves_distance(blocks: &mut FxHashMap<Cell, BlockState>) -> usize {
    let mut dist: FxHashMap<Cell, u8> = FxHashMap::default();
    let mut queue = VecDeque::new();
    for (&c, s) in blocks.iter() {
        if is_log(&s.name) {
            dist.insert(c, 0);
            queue.push_back(c);
        }
    }
    while let Some(c @ (x, y, z)) = queue.pop_front() {
        let d = dist[&c];
        if d >= 7 {
            continue;
        }
        for (dx, dy, dz) in DIRS {
            let n = (x + dx, y + dy, z + dz);
            if blocks.get(&n).is_some_and(is_leaves) && !dist.contains_key(&n) {
                dist.insert(n, d + 1);
                queue.push_back(n);
            }
        }
    }
    let mut changed = 0;
    for (c, s) in blocks.iter_mut() {
        if !is_leaves(s) {
            continue;
        }
        let d = dist.get(c).copied().unwrap_or(7).max(1);
        let before = s.clone();
        set_prop(s, "distance", &d.to_string());
        if d == 7 {
            set_prop(s, "persistent", "true");
        }
        changed += (*s != before) as usize;
    }
    changed
}

/// Kelp tops draw the same at every age; natural kelp is generated with age 20..=23 (KelpFeature), so one
/// of those beats the default 0. Returns the number of kelp changed.
pub fn kelp_age(blocks: &mut FxHashMap<Cell, BlockState>) -> usize {
    let mut changed = 0;
    for s in blocks.values_mut().filter(|s| s.name == "minecraft:kelp" && prop(s, "age") == Some("0")) {
        set_prop(s, "age", "21");
        changed += 1;
    }
    changed
}

fn is_log(name: &str) -> bool {
    ["_log", "_wood", "_stem", "_hyphae"].iter().any(|s| name.ends_with(s))
}

fn is_leaves(s: &BlockState) -> bool {
    s.name.ends_with("_leaves") && s.properties.iter().any(|(k, _)| k == "distance")
}

pub fn prop<'a>(s: &'a BlockState, key: &str) -> Option<&'a str> {
    s.properties.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

pub fn set_prop(s: &mut BlockState, key: &str, value: &str) {
    if let Some(p) = s.properties.iter_mut().find(|(k, _)| k == key) {
        p.1 = value.to_owned();
    }
}
