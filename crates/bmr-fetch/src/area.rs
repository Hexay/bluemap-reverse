//! A box of world columns (`--area x0,z0,x1,z1`, inclusive) and how it clips tile grids.

use std::str::FromStr;

use crate::grid::{Grid, Tile};

/// Inclusive column box; `min` ≤ `max` on both axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockBox {
    pub min: [i32; 2],
    pub max: [i32; 2],
}

impl BlockBox {
    /// Corners in any order.
    pub fn new(x0: i32, z0: i32, x1: i32, z1: i32) -> Self {
        Self { min: [x0.min(x1), z0.min(z1)], max: [x0.max(x1), z0.max(z1)] }
    }

    pub fn contains(&self, x: i32, z: i32) -> bool {
        (self.min[0]..=self.max[0]).contains(&x) && (self.min[1]..=self.max[1]).contains(&z)
    }

    pub fn grow(self, by: i32) -> Self {
        Self { min: self.min.map(|v| v - by), max: self.max.map(|v| v + by) }
    }

    pub fn intersect(self, o: Self) -> Option<Self> {
        let min = [self.min[0].max(o.min[0]), self.min[1].max(o.min[1])];
        let max = [self.max[0].min(o.max[0]), self.max[1].min(o.max[1])];
        (min[0] <= max[0] && min[1] <= max[1]).then_some(Self { min, max })
    }

    /// Tiles of `grid` overlapping the box.
    pub fn tiles(&self, grid: &Grid) -> impl Iterator<Item = Tile> {
        grid.tiles_in(self.min[0], self.min[1], self.max[0] - self.min[0] + 1, self.max[1] - self.min[1] + 1)
    }

    pub fn touches(&self, grid: &Grid, (tx, tz): Tile) -> bool {
        let ((ax, az), (bx, bz)) = (grid.tile_of(self.min[0], self.min[1]), grid.tile_of(self.max[0], self.max[1]));
        (ax..=bx).contains(&tx) && (az..=bz).contains(&tz)
    }
}

impl FromStr for BlockBox {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let v: Vec<i32> =
            s.split(',').map(|p| p.trim().parse().map_err(|e| format!("`{p}`: {e}"))).collect::<Result<_, String>>()?;
        match v[..] {
            [x0, z0, x1, z1] => Ok(Self::new(x0, z0, x1, z1)),
            _ => Err(format!("expected x0,z0,x1,z1, got {} value(s)", v.len())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_corners_in_any_order() {
        assert_eq!("10,-5,-20,7".parse(), Ok(BlockBox { min: [-20, -5], max: [10, 7] }));
        assert_eq!(" 1, 2 ,3,4".parse::<BlockBox>().unwrap(), BlockBox::new(1, 2, 3, 4));
        assert!("1,2,3".parse::<BlockBox>().is_err());
        assert!("1,2,3,x".parse::<BlockBox>().is_err());
    }

    #[test]
    fn grow_and_intersect() {
        let b = BlockBox::new(0, 0, 9, 9);
        assert_eq!(b.grow(2), BlockBox::new(-2, -2, 11, 11));
        assert_eq!(b.intersect(BlockBox::new(5, -3, 20, 4)), Some(BlockBox::new(5, 0, 9, 4)));
        assert_eq!(b.intersect(BlockBox::new(10, 0, 20, 9)), None);
    }

    #[test]
    fn clips_an_offset_grid() {
        let g = Grid { size: [32, 32], offset: [2, 2] };
        // x 2..=33 is tile 0, so a box ending at 33 stays in it and one ending at 34 reaches tile 1
        let b = BlockBox::new(1, 2, 33, 33);
        assert_eq!(b.tiles(&g).collect::<Vec<_>>(), [(-1, 0), (0, 0)]);
        assert!(b.touches(&g, (0, 0)) && b.touches(&g, (-1, 0)));
        assert!(!b.touches(&g, (1, 0)) && !b.touches(&g, (0, 1)) && !b.touches(&g, (-2, 0)));
        assert!(BlockBox::new(2, 2, 34, 2).touches(&g, (1, 0)));
    }
}
