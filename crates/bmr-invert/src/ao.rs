//! Ambient occlusion as evidence about hidden cells. BlueMap shades each vertex of a full face by the cells
//! around it in the layer the face looks into: the two side cells and the diagonal one, AO = 255 - 64 × the
//! number of them that are full blocks (measured on the vanilla-edited fixture, 2026-10-02: no exceptions
//! among air, liquids, plants and full cubes). Below the cave cutoff BlueMap drops the faces of rock that only
//! touches dark air, but the lit faces beside it still carry that rock in their AO.

use crate::face::{Cell, WorldFace, step};

/// A full face on a cell boundary, kept compact (a mirror holds ~10⁶ of them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AoFace {
    /// The cell the face looks into.
    front: Cell,
    normal_axis: u8,
    /// Occluder count per vertex, indexed by `corner_index`.
    counts: [u8; 4],
}

/// One vertex: its three shading cells and how many of them are full blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Corner {
    pub cells: [Cell; 3],
    pub occluders: u8,
}

/// What is known about a cell when solving corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Known {
    Occluder,
    Clear,
    /// Not observed: the corner may decide it.
    Unseen,
    /// Observed but not matched: the corner can't be used.
    Unsure,
}

/// In-plane axes of a face whose normal lies along `axis`.
fn plane(axis: u8) -> (usize, usize) {
    [(1, 2), (0, 2), (0, 1)][axis as usize]
}

fn unit(axis: usize, d: i32) -> Cell {
    let mut c = [0; 3];
    c[axis] = d;
    (c[0], c[1], c[2])
}

fn corner_index(du: i32, dw: i32) -> usize {
    usize::from(du > 0) * 2 + usize::from(dw > 0)
}

impl AoFace {
    /// `None` for partial faces, liquids and AO values off the 64-step scale.
    pub fn of(f: &WorldFace) -> Option<Self> {
        if f.texture.liquid().is_some() {
            return None;
        }
        let axis = (0..3).find(|&a| f.normal[a].abs() > 0.99)?;
        let v: [[i32; 3]; 4] = f.verts.map(|p| p.map(|c| c.round() as i32));
        let integral = f.verts.iter().zip(&v).all(|(p, q)| (0..3).all(|a| (p[a] - q[a] as f32).abs() < 1e-3));
        let (u, w) = plane(axis as u8);
        let span = |a: usize| v.iter().map(|p| p[a]).max().unwrap() - v.iter().map(|p| p[a]).min().unwrap();
        if !integral || span(u) != 1 || span(w) != 1 || span(axis) != 0 {
            return None;
        }
        let owner = f.owner();
        let front = step(owner, unit(axis, if f.normal[axis] > 0.0 { 1 } else { -1 }));
        let base = [owner.0, owner.1, owner.2];
        let mut counts = [u8::MAX; 4];
        for (p, ao) in v.iter().zip(f.ao) {
            let n = match ao {
                255 => 0,
                191 => 1,
                127 => 2,
                63 => 3,
                _ => return None,
            };
            let dir = |a: usize| if p[a] > base[a] { 1 } else { -1 };
            counts[corner_index(dir(u), dir(w))] = n;
        }
        (!counts.contains(&u8::MAX)).then_some(Self { front, normal_axis: axis as u8, counts })
    }

    pub fn corners(&self) -> impl Iterator<Item = Corner> + '_ {
        let (u, w) = plane(self.normal_axis);
        [(-1, -1), (-1, 1), (1, -1), (1, 1)].into_iter().map(move |(du, dw)| {
            let (a, b) = (step(self.front, unit(u, du)), step(self.front, unit(w, dw)));
            Corner { cells: [a, b, step(a, unit(w, dw))], occluders: self.counts[corner_index(du, dw)] }
        })
    }
}

/// Cells a corner decides: `(cell, full block?)` for its unseen cells when the known ones leave no choice.
pub fn decide(c: &Corner, known: impl Fn(Cell) -> Known) -> Vec<(Cell, bool)> {
    let states = c.cells.map(known);
    if states.contains(&Known::Unsure) {
        return Vec::new();
    }
    let occ = states.iter().filter(|&&s| s == Known::Occluder).count() as u8;
    let unseen = c.cells.iter().zip(states).filter(|(_, s)| *s == Known::Unseen).map(|(&cell, _)| cell);
    let n = states.iter().filter(|&&s| s == Known::Unseen).count() as u8;
    if occ == c.occluders {
        unseen.map(|cell| (cell, false)).collect()
    } else if occ + n == c.occluders {
        unseen.map(|cell| (cell, true)).collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
#[path = "ao_tests.rs"]
mod tests;
