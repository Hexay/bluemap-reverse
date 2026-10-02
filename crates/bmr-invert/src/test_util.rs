//! Hand-built keys, faces and library entries. Texture ids come from the process-global interner, so
//! nothing here (or in tests) may depend on their values or order: always compare through `signature`.

use bmr_world::BlockState;

use crate::face::{Cell, CellFaces, DIRS, FaceKey, Q, Tex, WorldFace, signature};
use crate::library::{Entry, Library, is_full_cube};

pub const STONE: &str = "minecraft:block/stone";
pub const WATER: &str = "minecraft:block/water_still";

pub fn key(tex: &str, mut verts: [[i16; 3]; 4]) -> FaceKey {
    verts.sort();
    FaceKey { texture: Tex::intern(tex), tinted: false, verts }
}

/// Corners (cyclic order) of the outer side of the unit cell facing `d`, in 1/Q units.
pub fn side_corners(d: Cell) -> [[i16; 3]; 4] {
    let q = Q as i16;
    let dir = [d.0, d.1, d.2];
    let a = dir.iter().position(|&c| c != 0).expect("unit direction");
    let (u, v) = ((a + 1) % 3, (a + 2) % 3);
    let corner = |pu: i16, pv: i16| {
        let mut p = [0; 3];
        p[a] = if dir[a] > 0 { q } else { 0 };
        p[u] = pu;
        p[v] = pv;
        p
    };
    [corner(0, 0), corner(q, 0), corner(q, q), corner(0, q)]
}

pub fn side(tex: &str, d: Cell) -> FaceKey {
    key(tex, side_corners(d))
}

/// The six full sides of a cube, `tex(d)` per direction.
pub fn cube_with(tex: impl Fn(Cell) -> &'static str) -> Vec<FaceKey> {
    signature(DIRS.iter().map(|&d| side(tex(d), d)).collect())
}

pub fn cube(tex: &'static str) -> Vec<FaceKey> {
    cube_with(|_| tex)
}

/// Horizontal quad across the whole cell at height `y` (1/Q units).
pub fn horizontal(tex: &str, y: i16) -> FaceKey {
    let q = Q as i16;
    key(tex, [[0, y, 0], [q, y, 0], [q, y, q], [0, y, q]])
}

/// Plant-style diagonal cross, shifted by (dx, dz) in 1/Q units.
pub fn cross(tex: &str, dx: i16, dz: i16) -> Vec<FaceKey> {
    let (lo, hi, q) = (8, 56, Q as i16);
    let at = |x: i16, y: i16, z: i16| [x + dx, y, z + dz];
    signature(vec![
        key(tex, [at(lo, 0, lo), at(lo, q, lo), at(hi, q, hi), at(hi, 0, hi)]),
        key(tex, [at(lo, 0, hi), at(lo, q, hi), at(hi, q, lo), at(hi, 0, lo)]),
    ])
}

/// `minecraft:name[k=v,…]` → state.
pub fn state(s: &str) -> BlockState {
    let (name, props) = s.split_once('[').map_or((s, ""), |(n, p)| (n, p.trim_end_matches(']')));
    let props = props
        .split(',')
        .filter(|p| !p.is_empty())
        .map(|p| p.split_once('=').expect("k=v"))
        .map(|(k, v)| (k.into(), v.into()));
    BlockState::new(name.into(), props.collect())
}

pub fn entry(s: &str, sig: Vec<FaceKey>) -> Entry {
    Entry {
        state: state(s),
        // from_entries zips sig with uvs: they must be the same length
        uvs: vec![[[0; 2]; 4]; sig.len()],
        full_cube: is_full_cube(&sig),
        sig: signature(sig),
        light: 0,
        liquid: None,
        overhang: Vec::new(),
        tint: None,
        default_distance: 0,
    }
}

pub fn library(entries: Vec<Entry>) -> Library {
    Library::from_entries(entries, 0)
}

pub fn observed(keys: Vec<FaceKey>) -> CellFaces {
    let mut c = CellFaces::default();
    c.keys = keys;
    c
}

/// World-space full side of `cell` facing `d`.
pub fn world_side(cell: Cell, d: Cell, tex: &str, color: [u8; 3], blocklight: u8) -> WorldFace {
    let base = [cell.0 as f32, cell.1 as f32, cell.2 as f32];
    WorldFace {
        verts: side_corners(d).map(|c| std::array::from_fn(|a| base[a] + c[a] as f32 / Q)),
        uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        normal: [d.0 as f32, d.1 as f32, d.2 as f32],
        texture: Tex::intern(tex),
        color,
        blocklight,
    }
}

/// PRBM tile assembled the way BlueMap emits it: a quad is (c0,c1,c2),(c0,c2,c3); push in material order.
#[derive(Default)]
pub struct TileBuilder(pub bmr_prbm::Tile);

impl TileBuilder {
    pub fn triangle(&mut self, material: u32, tri: [[f32; 3]; 3], normal: [i8; 3]) -> &mut Self {
        let t = &mut self.0;
        let start = t.position.len() as u32;
        t.position.extend(tri);
        t.normal.extend([normal; 3]);
        t.color.extend([[255; 3]; 3]);
        t.uv.extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]);
        t.ao.extend([255; 3]);
        t.blocklight.extend([0; 3]);
        t.sunlight.extend([15; 3]);
        match t.groups.last_mut() {
            Some(g) if g.material == material => g.count += 3,
            _ => t.groups.push(bmr_prbm::Group { material, start, count: 3 }),
        }
        self
    }

    pub fn quad(&mut self, material: u32, [c0, c1, c2, c3]: [[f32; 3]; 4], normal: [i8; 3]) -> &mut Self {
        self.triangle(material, [c0, c1, c2], normal).triangle(material, [c0, c2, c3], normal)
    }
}

/// Library entry ids by state string.
pub fn id(lib: &Library, s: &str) -> usize {
    lib.find(&state(s)).unwrap_or_else(|| panic!("{s} not in library"))
}
