//! Tile triangles → world-space quads, and quads grouped by the cell that owns them.

use bmr_prbm::{Face, Tile};
use rustc_hash::FxHashMap;

use super::key::{Cell, FaceKey, Q, Uv};
use crate::texture::Tex;

const UV_Q: f32 = 256.0;

/// A quad in world space.
pub struct WorldFace {
    pub verts: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub normal: [f32; 3],
    pub texture: Tex,
    /// Tint multiplier; `[255; 3]` = untinted.
    pub color: [u8; 3],
    pub blocklight: u8,
    /// Per vertex, 255 = unoccluded (see ao.rs).
    pub ao: [u8; 4],
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

    pub fn key_at(&self, cell: Cell) -> FaceKey {
        self.key_uv_at(cell).0
    }

    pub fn key_uv_at(&self, (bx, by, bz): Cell) -> (FaceKey, Uv) {
        let base = [bx as f32, by as f32, bz as f32];
        let mut corners: [([i16; 3], [i16; 2]); 4] = std::array::from_fn(|i| {
            let v = self.verts[i];
            (
                std::array::from_fn(|a| ((v[a] - base[a]) * Q).round() as i16),
                self.uv[i].map(|c| (c * UV_Q).round() as i16),
            )
        });
        corners.sort();
        let key = FaceKey { texture: self.texture, tinted: self.color != [255, 255, 255], verts: corners.map(|c| c.0) };
        (key, corners.map(|c| c.1))
    }
}

/// Interned texture per PRBM material index of one map.
pub fn texture_ids(resource_paths: &[String]) -> Vec<Tex> {
    resource_paths.iter().map(|p| Tex::intern(p)).collect()
}

/// A tile's quads in world space. BlueMap emits each quad as consecutive triangles (c0,c1,c2),(c0,c2,c3);
/// a triangle that does not pair up becomes a degenerate quad.
pub fn world_faces(tile: &Tile, [ox, oz]: [i32; 2], textures: &[Tex]) -> Vec<WorldFace> {
    let unknown = Tex::intern("?");
    let tris: Vec<Face> = tile.faces().collect();
    let mut out = Vec::with_capacity(tris.len() / 2);
    let mut i = 0;
    while i < tris.len() {
        let a = &tris[i];
        let pair = tris.get(i + 1).filter(|b| b.material == a.material && b.pos[0] == a.pos[0] && b.pos[1] == a.pos[2]);
        let (c3, uv3, ao3) = pair.map_or((a.pos[2], a.uv[2], a.ao[2]), |b| (b.pos[2], b.uv[2], b.ao[2]));
        i += if pair.is_some() { 2 } else { 1 };
        let verts = [a.pos[0], a.pos[1], a.pos[2], c3].map(|[x, y, z]| [x + ox as f32, y, z + oz as f32]);
        out.push(WorldFace {
            verts,
            uv: [a.uv[0], a.uv[1], a.uv[2], uv3],
            normal: a.normal.map(|c| c as f32 / 127.0),
            texture: textures.get(a.material as usize).copied().unwrap_or(unknown),
            color: a.color,
            blocklight: a.blocklight.max(0) as u8,
            ao: [a.ao[0], a.ao[1], a.ao[2], ao3],
        });
    }
    out
}

/// What one cell shows: its face keys (matching), their UVs (tie-breaks), the mean tint of its tinted
/// faces and the dimmest block light on them (a light source lights all its faces; a neighbour's light
/// reaches only some, so the minimum measures the block's own emission).
#[derive(Default)]
pub struct CellFaces {
    pub keys: Vec<FaceKey>,
    /// Every pushed face with its UVs; not pruned when overhang is credited away from `keys`.
    pub uvs: Vec<(FaceKey, Uv)>,
    light: Option<u8>,
    tint_sum: [u32; 3],
    tint_n: u32,
}

impl CellFaces {
    /// `f` keyed relative to `cell`.
    pub fn push(&mut self, f: &WorldFace, cell: Cell) {
        let (key, uv) = f.key_uv_at(cell);
        // water tint is biome noise, never evidence about the block
        if key.tinted && key.liquid().is_none() {
            for a in 0..3 {
                self.tint_sum[a] += f.color[a] as u32;
            }
            self.tint_n += 1;
        }
        if key.liquid().is_none() {
            self.light = Some(self.light.map_or(f.blocklight, |l| l.min(f.blocklight)));
            self.uvs.push((key, uv));
        }
        self.keys.push(key);
    }

    pub fn tint(&self) -> Option<[u8; 3]> {
        (self.tint_n > 0).then(|| self.tint_sum.map(|s| (s / self.tint_n) as u8))
    }

    pub fn light(&self) -> u8 {
        self.light.unwrap_or(0)
    }

    pub fn merge(&mut self, other: CellFaces) {
        self.keys.extend(other.keys);
        self.uvs.extend(other.uvs);
        self.light = match (self.light, other.light) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        for a in 0..3 {
            self.tint_sum[a] += other.tint_sum[a];
        }
        self.tint_n += other.tint_n;
    }
}

/// Group faces by owning cell, keyed relative to that cell.
pub fn faces_by_cell(faces: &[WorldFace]) -> FxHashMap<Cell, CellFaces> {
    let mut out: FxHashMap<Cell, CellFaces> = FxHashMap::default();
    for f in faces {
        let cell = f.owner();
        out.entry(cell).or_default().push(f, cell);
    }
    out
}
