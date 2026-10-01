//! Parsed hires tile. Vertex arrays are kept raw (lossless); triangles are 3 consecutive vertices.
//! Semantics per attribute: docs/research/01 §2.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Group {
    pub material: u32,
    /// In vertices.
    pub start: u32,
    pub count: u32,
}

#[derive(Debug, Default)]
pub struct Tile {
    /// x/z relative to the tile's min corner, y absolute world y.
    pub position: Vec<[f32; 3]>,
    /// `(byte)(n*128-0.5)` of the geometric normal, same for all 3 vertices.
    pub normal: Vec<[i8; 3]>,
    /// Tint multiplier (255 = untinted), per face.
    pub color: Vec<[u8; 3]>,
    /// Texture-local, v=0 at the image top, spans one animation frame.
    pub uv: Vec<[f32; 2]>,
    /// Per vertex, 255 = unoccluded.
    pub ao: Vec<u8>,
    pub blocklight: Vec<i8>,
    pub sunlight: Vec<i8>,
    /// Sorted by material, covering every vertex exactly once.
    pub groups: Vec<Group>,
}

/// One triangle with its per-face values collapsed.
#[derive(Debug, Clone, Copy)]
pub struct Face {
    pub material: u32,
    pub pos: [[f32; 3]; 3],
    pub uv: [[f32; 2]; 3],
    pub ao: [u8; 3],
    pub color: [u8; 3],
    pub normal: [i8; 3],
    pub blocklight: i8,
    pub sunlight: i8,
}

impl Tile {
    pub fn face_count(&self) -> usize {
        self.position.len() / 3
    }

    pub fn faces(&self) -> impl Iterator<Item = Face> + '_ {
        self.groups.iter().flat_map(move |g| {
            let first = g.start as usize / 3;
            (first..first + g.count as usize / 3).map(move |i| self.face(i, g.material))
        })
    }

    fn face(&self, i: usize, material: u32) -> Face {
        let v = i * 3;
        Face {
            material,
            pos: [self.position[v], self.position[v + 1], self.position[v + 2]],
            uv: [self.uv[v], self.uv[v + 1], self.uv[v + 2]],
            ao: [self.ao[v], self.ao[v + 1], self.ao[v + 2]],
            color: self.color[v],
            normal: self.normal[v],
            blocklight: self.blocklight[v],
            sunlight: self.sunlight[v],
        }
    }
}
