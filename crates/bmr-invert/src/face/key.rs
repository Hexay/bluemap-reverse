//! Position-independent face keys and the signatures built from them.

use crate::texture::Tex;

/// Cell-local coordinates are in 1/Q block units.
pub const Q: f32 = 64.0;

pub type Cell = (i32, i32, i32);

pub const DIRS: [Cell; 6] = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];

pub fn step((x, y, z): Cell, (dx, dy, dz): Cell) -> Cell {
    (x + dx, y + dy, z + dz)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Liquid {
    Water,
    Lava,
}

impl Liquid {
    /// Source-block state name.
    pub fn block(self) -> &'static str {
        match self {
            Liquid::Water => "minecraft:water",
            Liquid::Lava => "minecraft:lava",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceKey {
    pub texture: Tex,
    pub tinted: bool,
    /// Sorted, cell-local, quantized to 1/Q.
    pub verts: [[i16; 3]; 4],
}

impl FaceKey {
    /// All vertices on one outer plane of the cell (x/y/z = 0 or 1): a face neighbour culling can remove.
    pub fn on_boundary(&self) -> bool {
        self.boundary_dir().is_some()
    }

    /// Lies in a plane perpendicular to an axis: the only faces a model can give a cullface.
    pub fn axis_aligned(&self) -> bool {
        (0..3).any(|axis| self.verts.iter().all(|v| v[axis] == self.verts[0][axis]))
    }

    /// Offset to the neighbour cell across the outer plane this face lies on, if any.
    pub fn boundary_dir(&self) -> Option<Cell> {
        (0..3).find_map(|axis| {
            let c = self.verts[0][axis];
            if !((c == 0 || c == Q as i16) && self.verts.iter().all(|v| v[axis] == c)) {
                return None;
            }
            let s = if c == 0 { -1 } else { 1 };
            Some(match axis {
                0 => (s, 0, 0),
                1 => (0, s, 0),
                _ => (0, 0, s),
            })
        })
    }

    pub fn liquid(&self) -> Option<Liquid> {
        self.texture.liquid()
    }

    /// Direction a liquid face faces. Liquid surfaces sit below the cell top (14/16 for a lone source)
    /// so they are classified by plane, not by `boundary_dir`: horizontal at y=0 is the bottom, any
    /// other horizontal face is the surface; vertical faces always lie on a cell side; anything else is a
    /// flowing liquid's sloped surface.
    pub fn liquid_dir(&self) -> Option<Cell> {
        let same = |a: usize| self.verts.iter().all(|v| v[a] == self.verts[0][a]);
        if same(1) {
            return Some(if self.verts[0][1] == 0 { (0, -1, 0) } else { (0, 1, 0) });
        }
        self.boundary_dir().or(Some((0, 1, 0)))
    }

    /// Covers a whole outer plane of the cell.
    pub fn full_side(&self) -> bool {
        self.boundary_dir().is_some() && {
            let q = Q as i16;
            let spans = |a: usize| {
                let (lo, hi) =
                    (self.verts.iter().map(|v| v[a]).min().unwrap(), self.verts.iter().map(|v| v[a]).max().unwrap());
                lo == 0 && hi == q
            };
            (0..3).filter(|&a| spans(a)).count() >= 2
        }
    }

    fn translated(&self, dx: i16, dz: i16) -> Self {
        let mut k = *self;
        for v in &mut k.verts {
            v[0] -= dx;
            v[2] -= dz;
        }
        k.verts.sort();
        k
    }
}

/// Texture coordinates of a key's vertices, in the key's (sorted) vertex order, in 1/UV_Q texture units.
pub type Uv = [[i16; 2]; 4];

/// Sorted multiset of keys (the cell signature).
pub fn signature(mut faces: Vec<FaceKey>) -> Vec<FaceKey> {
    faces.sort();
    faces
}

/// Signature plus its UVs, index for index.
pub fn signature_uv(mut faces: Vec<(FaceKey, Uv)>) -> (Vec<FaceKey>, Vec<Uv>) {
    faces.sort();
    faces.into_iter().unzip()
}

/// Signature with its x/z bounding-box minimum moved to 0 (for tolerant offset matching).
pub fn corner_aligned(sig: &[FaceKey]) -> Vec<FaceKey> {
    let min_x = sig.iter().flat_map(|k| k.verts.iter()).map(|v| v[0]).min().unwrap_or(0);
    let min_z = sig.iter().flat_map(|k| k.verts.iter()).map(|v| v[2]).min().unwrap_or(0);
    signature(sig.iter().map(|k| k.translated(min_x, min_z)).collect())
}

/// Same texture/tint and every vertex within ±1 unit (offset rounding jitter).
pub fn close(a: &FaceKey, b: &FaceKey) -> bool {
    a.texture == b.texture
        && a.tinted == b.tinted
        && a.verts.iter().zip(&b.verts).all(|(p, q)| (0..3).all(|i| (p[i] - q[i]).abs() <= 1))
}

/// Signature translated so its x/z bounding box is centred: undoes BlueMap's random plant offsets.
pub fn normalized(sig: &[FaceKey]) -> Vec<FaceKey> {
    let (mut min_x, mut max_x, mut min_z, mut max_z) = (i16::MAX, i16::MIN, i16::MAX, i16::MIN);
    for v in sig.iter().flat_map(|k| k.verts.iter()) {
        (min_x, max_x, min_z, max_z) = (min_x.min(v[0]), max_x.max(v[0]), min_z.min(v[2]), max_z.max(v[2]));
    }
    let (dx, dz) = ((min_x + max_x) / 2 - Q as i16 / 2, (min_z + max_z) / 2 - Q as i16 / 2);
    signature(sig.iter().map(|k| k.translated(dx, dz)).collect())
}
