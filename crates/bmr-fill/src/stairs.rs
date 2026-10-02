//! A stair corner draws the same geometry as its twin (facing f + *_left ≡ facing ccw(f) + *_right), so the
//! tiles leave the pair open. The game derives the shape from the neighbouring stairs
//! (StairBlock.getStairsShape): keep the twin whose shape its neighbours would produce.

use rustc_hash::FxHashMap;

use bmr_invert::face::Cell;
use bmr_world::BlockState;

use crate::rules::{prop, set_prop};

const FACINGS: [(&str, Cell); 4] =
    [("north", (0, 0, -1)), ("east", (1, 0, 0)), ("south", (0, 0, 1)), ("west", (-1, 0, 0))];

/// Returns the number of stairs switched to their twin.
pub fn resolve_corners(blocks: &mut FxHashMap<Cell, BlockState>) -> usize {
    let corners: Vec<Cell> = blocks
        .iter()
        .filter(|(_, s)| is_stairs(s) && prop(s, "shape").is_some_and(|v| v != "straight"))
        .map(|(c, _)| *c)
        .collect();
    let mut changed = 0;
    // a neighbour's flip can change what fits, so settle twice
    for _ in 0..2 {
        let flips: Vec<(Cell, BlockState)> = corners
            .iter()
            .filter_map(|&c| {
                let s = &blocks[&c];
                let t = twin(s)?;
                (!consistent(blocks, c, s) && consistent(blocks, c, &t)).then_some((c, t))
            })
            .collect();
        if flips.is_empty() {
            break;
        }
        changed += flips.len();
        blocks.extend(flips);
    }
    changed
}

fn consistent(blocks: &FxHashMap<Cell, BlockState>, at: Cell, s: &BlockState) -> bool {
    prop(s, "shape") == Some(shape(blocks, at, s))
}

/// Vanilla StairBlock.getStairsShape for `s` placed at `at`.
fn shape(blocks: &FxHashMap<Cell, BlockState>, at: Cell, s: &BlockState) -> &'static str {
    let (Some(f), Some(half)) = (prop(s, "facing"), prop(s, "half")) else { return "straight" };
    let same_half = |c: Cell| blocks.get(&c).filter(|n| is_stairs(n) && prop(n, "half") == Some(half));
    let can_take = |d: &str| {
        blocks
            .get(&step(at, d))
            .is_none_or(|n| !is_stairs(n) || prop(n, "facing") != Some(f) || prop(n, "half") != Some(half))
    };
    if let Some(d1) = same_half(step(at, f)).and_then(|n| prop(n, "facing"))
        && axis(d1) != axis(f)
        && can_take(opposite(d1))
    {
        return if d1 == ccw(f) { "outer_left" } else { "outer_right" };
    }
    if let Some(d2) = same_half(step(at, opposite(f))).and_then(|n| prop(n, "facing"))
        && axis(d2) != axis(f)
        && can_take(d2)
    {
        return if d2 == ccw(f) { "inner_left" } else { "inner_right" };
    }
    "straight"
}

fn twin(s: &BlockState) -> Option<BlockState> {
    let f = prop(s, "facing")?;
    let (facing, shape) = match prop(s, "shape")? {
        "inner_left" => (ccw(f), "inner_right"),
        "outer_left" => (ccw(f), "outer_right"),
        "inner_right" => (cw(f), "inner_left"),
        "outer_right" => (cw(f), "outer_left"),
        _ => return None,
    };
    let mut t = s.clone();
    set_prop(&mut t, "facing", facing);
    set_prop(&mut t, "shape", shape);
    Some(t)
}

fn is_stairs(s: &BlockState) -> bool {
    s.name.ends_with("_stairs")
}

fn index(d: &str) -> usize {
    FACINGS.iter().position(|(n, _)| *n == d).unwrap_or(0)
}

fn ccw(d: &str) -> &'static str {
    FACINGS[(index(d) + 3) % 4].0
}

fn cw(d: &str) -> &'static str {
    FACINGS[(index(d) + 1) % 4].0
}

fn opposite(d: &str) -> &'static str {
    FACINGS[(index(d) + 2) % 4].0
}

fn axis(d: &str) -> usize {
    index(d) % 2
}

fn step((x, y, z): Cell, d: &str) -> Cell {
    let (dx, dy, dz) = FACINGS[index(d)].1;
    (x + dx, y + dy, z + dz)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stairs(facing: &str, shape: &str) -> BlockState {
        let props = [("facing", facing), ("half", "bottom"), ("shape", shape), ("waterlogged", "false")];
        BlockState::new(
            "minecraft:oak_stairs".into(),
            props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        )
    }

    #[test]
    fn corner_takes_the_twin_its_neighbours_produce() {
        // south-facing stair at the origin with an east-facing stair in front (north of it): inner corner.
        let mut blocks: FxHashMap<Cell, BlockState> = FxHashMap::default();
        blocks.insert((0, 0, -1), stairs("east", "straight"));
        let truth = stairs("south", shape(&blocks, (0, 0, 0), &stairs("south", "straight")));
        let observed = twin(&truth).unwrap();
        blocks.insert((0, 0, 0), observed);
        assert_eq!(resolve_corners(&mut blocks), 1);
        assert_eq!(blocks[&(0, 0, 0)], truth);
    }
}
