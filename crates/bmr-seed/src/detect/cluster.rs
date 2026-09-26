//! Connected components of marker blocks: points within `link` blocks (Chebyshev, XZ) join one cluster.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bbox {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

impl Bbox {
    pub fn at(p: [i32; 3]) -> Self {
        Self { min: p, max: p }
    }

    fn add(&mut self, p: [i32; 3]) {
        self.min = std::array::from_fn(|i| self.min[i].min(p[i]));
        self.max = std::array::from_fn(|i| self.max[i].max(p[i]));
    }

    /// Extent along x and z (inclusive).
    pub fn span_xz(&self) -> (i32, i32) {
        (self.max[0] - self.min[0] + 1, self.max[2] - self.min[2] + 1)
    }

    pub fn center_xz(&self) -> (i32, i32) {
        ((self.min[0] + self.max[0]).div_euclid(2), (self.min[2] + self.max[2]).div_euclid(2))
    }
}

#[derive(Clone, Debug)]
pub struct Cluster {
    pub bbox: Bbox,
    pub points: Vec<[i32; 3]>,
}

pub fn clusters(points: &[[i32; 3]], link: i32) -> Vec<Cluster> {
    let cell = |p: [i32; 3]| (p[0].div_euclid(link), p[2].div_euclid(link));
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, &p) in points.iter().enumerate() {
        grid.entry(cell(p)).or_default().push(i);
    }
    let mut seen = vec![false; points.len()];
    let mut out = Vec::new();
    for start in 0..points.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![start];
        let mut members = Vec::new();
        while let Some(i) = stack.pop() {
            let p = points[i];
            members.push(p);
            let (gx, gz) = cell(p);
            for dx in -1..=1 {
                for dz in -1..=1 {
                    for &j in grid.get(&(gx + dx, gz + dz)).into_iter().flatten() {
                        let q = points[j];
                        if !seen[j] && (p[0] - q[0]).abs() <= link && (p[2] - q[2]).abs() <= link {
                            seen[j] = true;
                            stack.push(j);
                        }
                    }
                }
            }
        }
        let mut bbox = Bbox::at(members[0]);
        members.iter().for_each(|&p| bbox.add(p));
        out.push(Cluster { bbox, points: members });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_by_gap() {
        let pts = [[0, 60, 0], [2, 60, 1], [3, 61, 3], [20, 60, 20], [21, 60, 20]];
        let mut c = clusters(&pts, 2);
        c.sort_by_key(|c| c.points.len());
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].bbox, Bbox { min: [0, 60, 0], max: [3, 61, 3] });
    }
}
