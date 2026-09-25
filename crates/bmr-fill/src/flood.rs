//! Which unseen cells are solid. BlueMap drops a face only when the neighbour it faces is a full opaque
//! (culling) block, or below the cave cut-off in darkness. So an opaque cell with no drawn faces has only
//! opaque neighbours: flood from the occluder evidence through faceless cells.
//! Cave caveat: below `remove-caves-below-y`, dark air also loses faces, so caves read as solid.

use std::collections::{HashMap, HashSet, VecDeque};

use bmr_invert::face::Cell;

pub struct Bounds<'a> {
    /// World (x, z) inside the rendered area.
    pub column: &'a (dyn Fn(i32, i32) -> bool + Sync),
    pub min_y: i32,
    pub max_y: i32,
}

impl Bounds<'_> {
    pub fn contains(&self, (x, y, z): Cell) -> bool {
        (self.min_y..=self.max_y).contains(&y) && (self.column)(x, z)
    }
}

pub const NEIGHBOURS: [Cell; 6] = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];

/// Hidden solid cells: occluders plus everything reachable from them through unobserved cells.
pub fn hidden_solids<V>(observed: &HashMap<Cell, V>, occluders: &HashSet<Cell>, bounds: &Bounds) -> HashSet<Cell> {
    let mut solid: HashSet<Cell> = occluders.iter().copied().filter(|&c| bounds.contains(c)).collect();
    let mut queue: VecDeque<Cell> = solid.iter().copied().collect();
    while let Some((x, y, z)) = queue.pop_front() {
        for (dx, dy, dz) in NEIGHBOURS {
            let n = (x + dx, y + dy, z + dz);
            if bounds.contains(n) && !observed.contains_key(&n) && solid.insert(n) {
                queue.push_back(n);
            }
        }
    }
    solid
}
