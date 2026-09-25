//! Quads → (block cell, position-independent key). UVs are ignored on purpose: BlueMap picks
//! rotated/mirrored model variants by position hash, which changes UVs but not geometry or texture.
//! Keys are per quad, not per triangle: a rotated variant moves the quad's diagonal.

use std::collections::HashMap;
use std::sync::Arc;

use bmr_prbm::{Face, Tile};

/// Cell-local coordinates are in 1/Q block units.
pub const Q: f32 = 64.0;

pub type Cell = (i32, i32, i32);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceKey {
    pub texture: Arc<str>,
    pub tinted: bool,
    /// Sorted, cell-local, quantized to 1/Q.
    pub verts: [[i16; 3]; 4],
}

impl FaceKey {
    /// All vertices on one outer plane of the cell (x/y/z = 0 or 1): a face neighbour culling can remove.
    pub fn on_boundary(&self) -> bool {
        self.boundary_dir().is_some()
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

    /// Covers a whole outer plane of the cell.
    pub fn full_side(&self) -> bool {
        self.boundary_dir().is_some() && {
            let q = Q as i16;
            let spans = |a: usize| {
                let (lo, hi) = (self.verts.iter().map(|v| v[a]).min().unwrap(), self.verts.iter().map(|v| v[a]).max().unwrap());
                lo == 0 && hi == q
            };
            (0..3).filter(|&a| spans(a)).count() >= 2
        }
    }

    fn translated(&self, dx: i16, dz: i16) -> Self {
        let mut k = self.clone();
        for v in &mut k.verts {
            v[0] -= dx;
            v[2] -= dz;
        }
        k.verts.sort();
        k
    }
}

/// A quad in world space.
pub struct WorldFace {
    pub verts: [[f32; 3]; 4],
    pub normal: [f32; 3],
    pub texture: Arc<str>,
    /// Tint multiplier; `[255; 3]` = untinted.
    pub color: [u8; 3],
}

impl WorldFace {
    pub fn centroid(&self) -> [f32; 3] {
        std::array::from_fn(|a| self.verts.iter().map(|v| v[a]).sum::<f32>() / 4.0)
    }

    /// The cell just behind the face along its normal: a face on a cell boundary belongs to the block
    /// it faces out of.
    pub fn owner(&self) -> Cell {
        let c = self.centroid();
        let p: [i32; 3] = std::array::from_fn(|a| (c[a] - self.normal[a] * 1e-3).floor() as i32);
        (p[0], p[1], p[2])
    }

    pub fn key_at(&self, (bx, by, bz): Cell) -> FaceKey {
        let base = [bx as f32, by as f32, bz as f32];
        let mut verts = self.verts.map(|v| std::array::from_fn(|a| ((v[a] - base[a]) * Q).round() as i16));
        verts.sort();
        FaceKey { texture: self.texture.clone(), tinted: self.color != [255, 255, 255], verts }
    }
}

/// Texture names by PRBM material index, interned once per map.
pub fn texture_names(textures: &[bmr_prbm::Texture]) -> Vec<Arc<str>> {
    textures.iter().map(|t| Arc::from(t.resource_path.as_str())).collect()
}

/// A tile's quads in world space. BlueMap emits each quad as consecutive triangles (c0,c1,c2),(c0,c2,c3);
/// a triangle that does not pair up becomes a degenerate quad.
pub fn world_faces(tile: &Tile, [ox, oz]: [i32; 2], names: &[Arc<str>]) -> Vec<WorldFace> {
    let tris: Vec<Face> = tile.faces().collect();
    let mut out = Vec::with_capacity(tris.len() / 2);
    let mut i = 0;
    while i < tris.len() {
        let a = &tris[i];
        let pair = tris.get(i + 1).filter(|b| b.material == a.material && b.pos[0] == a.pos[0] && b.pos[1] == a.pos[2]);
        let c3 = pair.map_or(a.pos[2], |b| b.pos[2]);
        i += if pair.is_some() { 2 } else { 1 };
        let verts = [a.pos[0], a.pos[1], a.pos[2], c3].map(|[x, y, z]| [x + ox as f32, y, z + oz as f32]);
        out.push(WorldFace {
            verts,
            normal: a.normal.map(|c| c as f32 / 127.0),
            texture: names.get(a.material as usize).cloned().unwrap_or_else(|| Arc::from("?")),
            color: a.color,
        });
    }
    out
}

/// What one cell shows: its face keys and the mean tint of its tinted faces.
#[derive(Default)]
pub struct CellFaces {
    pub keys: Vec<FaceKey>,
    tint_sum: [u32; 3],
    tint_n: u32,
}

impl CellFaces {
    pub fn push(&mut self, f: &WorldFace, key: FaceKey) {
        if key.tinted {
            for a in 0..3 {
                self.tint_sum[a] += f.color[a] as u32;
            }
            self.tint_n += 1;
        }
        self.keys.push(key);
    }

    pub fn tint(&self) -> Option<[u8; 3]> {
        (self.tint_n > 0).then(|| self.tint_sum.map(|s| (s / self.tint_n) as u8))
    }

    pub fn merge(&mut self, other: CellFaces) {
        self.keys.extend(other.keys);
        for a in 0..3 {
            self.tint_sum[a] += other.tint_sum[a];
        }
        self.tint_n += other.tint_n;
    }
}

/// Group faces by owning cell, keyed relative to that cell.
pub fn faces_by_cell(faces: &[WorldFace]) -> HashMap<Cell, CellFaces> {
    let mut out: HashMap<Cell, CellFaces> = HashMap::new();
    for f in faces {
        let cell = f.owner();
        out.entry(cell).or_default().push(f, f.key_at(cell));
    }
    out
}

/// Sorted multiset of keys (the cell signature).
pub fn signature(mut faces: Vec<FaceKey>) -> Vec<FaceKey> {
    faces.sort();
    faces
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
