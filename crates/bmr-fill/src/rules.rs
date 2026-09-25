//! Properties the render does not show but game rules determine.

use std::collections::{HashMap, VecDeque};

use bmr_invert::face::{Cell, DIRS};
use bmr_world::BlockState;

/// Leaves `distance` = steps to the nearest log through leaves (1..=7, vanilla #logs ≈ logs, woods,
/// stems, hyphae). Unreachable leaves (distance 7) must be persistent, or they would have decayed.
/// Returns the number of leaves changed.
pub fn leaves_distance(blocks: &mut HashMap<Cell, BlockState>) -> usize {
    let mut dist: HashMap<Cell, u8> = HashMap::new();
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
            if blocks.get(&n).is_some_and(|s| is_leaves(s)) && !dist.contains_key(&n) {
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

fn is_log(name: &str) -> bool {
    ["_log", "_wood", "_stem", "_hyphae"].iter().any(|s| name.ends_with(s))
}

fn is_leaves(s: &BlockState) -> bool {
    s.name.ends_with("_leaves") && s.properties.iter().any(|(k, _)| k == "distance")
}

fn set_prop(s: &mut BlockState, key: &str, value: &str) {
    if let Some(p) = s.properties.iter_mut().find(|(k, _)| k == key) {
        p.1 = value.to_owned();
    }
}
