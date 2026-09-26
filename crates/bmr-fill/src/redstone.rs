//! `powered` of doors, trapdoors, fence gates, heads and note blocks does not render; the game sets it
//! when a neighbour feeds redstone power into the block. Sources are read off the reconstruction (wire
//! power comes from its tint). Only direct feeds count, not power relayed through a solid block.

use rustc_hash::FxHashMap;

use bmr_invert::face::{Cell, DIRS};
use bmr_world::BlockState;

use crate::rules::{prop, set_prop};

const TARGETS: [&str; 5] = ["_door", "_trapdoor", "_fence_gate", "_head", "_skull"];

/// Returns the number of blocks whose `powered` changed.
pub fn powered(blocks: &mut FxHashMap<Cell, BlockState>) -> usize {
    let is_target = |s: &BlockState| {
        prop(s, "powered").is_some() && (s.name == "minecraft:note_block" || TARGETS.iter().any(|t| s.name.ends_with(t)))
    };
    let mut on: FxHashMap<Cell, bool> =
        blocks.iter().filter(|(_, s)| is_target(s)).map(|(&c, _)| (c, fed(blocks, c))).collect();
    // a door's halves share `powered`
    let doors: Vec<(Cell, Cell)> = on
        .keys()
        .filter(|c| blocks[c].name.ends_with("_door") && prop(&blocks[c], "half") == Some("lower"))
        .map(|&(x, y, z)| ((x, y, z), (x, y + 1, z)))
        .collect();
    for (lower, upper) in doors {
        let both = on[&lower] || on.get(&upper).copied().unwrap_or(false);
        on.insert(lower, both);
        if on.contains_key(&upper) {
            on.insert(upper, both);
        }
    }
    let mut changed = 0;
    for (c, p) in on {
        let v = if p { "true" } else { "false" };
        let s = blocks.get_mut(&c).expect("target");
        if prop(s, "powered") != Some(v) {
            set_prop(s, "powered", v);
            changed += 1;
        }
    }
    changed
}

/// A neighbour powers the block at `at`.
fn fed(blocks: &FxHashMap<Cell, BlockState>, at: Cell) -> bool {
    DIRS.iter().any(|&(dx, dy, dz)| {
        let from = (at.0 + dx, at.1 + dy, at.2 + dz);
        blocks.get(&from).is_some_and(|s| feeds(s, (-dx, -dy, -dz)))
    })
}

/// Source `s` sends power in direction `d` (from itself towards the target).
fn feeds(s: &BlockState, d: Cell) -> bool {
    let name = s.name.trim_start_matches("minecraft:");
    let is = |k: &str, v: &str| prop(s, k) == Some(v);
    let facing = prop(s, "facing").and_then(dir);
    match name {
        "redstone_block" => true,
        "redstone_torch" => is("lit", "true") && d != (0, -1, 0),
        "redstone_wall_torch" => is("lit", "true") && facing.is_some_and(|f| d != neg(f)),
        "lever" => is("powered", "true"),
        "redstone_wire" => {
            prop(s, "power").is_some_and(|p| p != "0")
                && (d == (0, -1, 0) || horizontal_name(d).is_some_and(|n| prop(s, n).is_some_and(|v| v != "none")))
        }
        // output is on the side opposite `facing`
        "repeater" | "comparator" | "observer" => is("powered", "true") && facing.is_some_and(|f| d == neg(f)),
        "daylight_detector" | "target" | "sculk_sensor" | "calibrated_sculk_sensor" => prop(s, "power").is_some_and(|p| p != "0"),
        _ if name.ends_with("_button") || name.ends_with("pressure_plate") => {
            is("powered", "true") || prop(s, "power").is_some_and(|p| p != "0")
        }
        _ => false,
    }
}

fn dir(name: &str) -> Option<Cell> {
    Some(match name {
        "north" => (0, 0, -1),
        "south" => (0, 0, 1),
        "west" => (-1, 0, 0),
        "east" => (1, 0, 0),
        "up" => (0, 1, 0),
        "down" => (0, -1, 0),
        _ => return None,
    })
}

fn horizontal_name(d: Cell) -> Option<&'static str> {
    ["north", "south", "west", "east"].into_iter().find(|n| dir(n) == Some(d))
}

fn neg((x, y, z): Cell) -> Cell {
    (-x, -y, -z)
}
