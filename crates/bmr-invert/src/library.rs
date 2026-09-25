//! Face-signature library learned from BlueMap's render of the vanilla debug world
//! (every state once, isolated, at y=70 on odd x/z). See docs/plan.md "Inversion by learned signatures".

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use anyhow::{Context, Result};
use bmr_fetch::LocalMap;
use bmr_world::{BlockRegistry, BlockState, World};

use crate::face::{CellFaces, Cell, FaceKey, WorldFace, normalized, signature, texture_names, world_faces};

const DEBUG_Y: i32 = 70;

pub struct Entry {
    pub state: BlockState,
    /// In-cell faces only.
    pub sig: Vec<FaceKey>,
    /// Faces owned by a neighbouring cell: (offset to that cell, key relative to it).
    pub overhang: Vec<(Cell, FaceKey)>,
    /// Mean tint of tinted faces as rendered in the debug world (plains biome; redstone by power).
    pub tint: Option<[u8; 3]>,
    /// Number of properties differing from the block's default state (tie-break prior).
    pub default_distance: u32,
    /// Renders all 6 sides as full outer quads, like an opaque cube (stone, dirt, planks, glass…).
    pub full_cube: bool,
}

#[derive(Debug, Default)]
pub struct BuildStats {
    pub states: usize,
    /// Faces owned by a cell other than their block's (geometry reaching outside the block).
    pub overhang_faces: usize,
    pub overhang_states: usize,
}

pub struct Library {
    pub entries: Vec<Entry>,
    pub data_version: i32,
    pub stats: BuildStats,
    exact: HashMap<Vec<FaceKey>, Vec<usize>>,
    exact_norm: HashMap<Vec<FaceKey>, Vec<usize>>,
    by_texture: HashMap<Arc<str>, Vec<usize>>,
}

impl Library {
    pub fn build(map: &LocalMap, world: &World, registry: &BlockRegistry) -> Result<Self> {
        let textures = bmr_prbm::parse_textures(&map.textures_json()?)?;
        let names = texture_names(&textures);
        let mut faces: Vec<WorldFace> = Vec::new();
        for t in map.tiles(0) {
            let tile = bmr_prbm::parse(&map.tile_bytes(0, t)?)?;
            faces.extend(world_faces(&tile, map.hires_origin(t), &names));
        }
        let mut by_anchor: HashMap<Cell, CellFaces> = HashMap::new();
        let mut overhang: HashMap<Cell, Vec<(Cell, FaceKey)>> = HashMap::new();
        for f in &faces {
            let (a, o) = (anchor(f), f.owner());
            if o == a {
                by_anchor.entry(a).or_default().push(f, f.key_at(a));
            } else {
                overhang.entry(a).or_default().push(((o.0 - a.0, o.1 - a.1, o.2 - a.2), f.key_at(o)));
            }
        }
        // states whose every face overhangs still need an anchor entry
        for a in overhang.keys() {
            by_anchor.entry(*a).or_default();
        }

        let mut chunks = HashMap::new();
        for r in world.regions()? {
            chunks.extend(world.read_region(r)?);
        }
        let data_version = chunks.values().next().context("debug world has no chunks")?.data_version;

        let mut lib = Self {
            entries: Vec::new(),
            data_version,
            stats: BuildStats::default(),
            exact: HashMap::new(),
            exact_norm: HashMap::new(),
            by_texture: HashMap::new(),
        };
        let mut anchors: Vec<_> = by_anchor.into_iter().collect();
        anchors.sort_by_key(|(c, _)| *c);
        for ((x, y, z), cell) in anchors {
            let chunk = chunks.get(&(x.div_euclid(16), z.div_euclid(16)));
            let Some(state) = chunk.and_then(|c| c.block(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize))
            else {
                continue;
            };
            if state.is_air() {
                continue;
            }
            let overhang = overhang.remove(&(x, y, z)).unwrap_or_default();
            if !overhang.is_empty() {
                lib.stats.overhang_faces += overhang.len();
                lib.stats.overhang_states += 1;
            }
            let default_distance = registry.get(&state.name).map_or(0, |b| {
                state.properties.iter().filter(|p| !b.default.contains(p)).count() as u32
            });
            let tint = cell.tint();
            let sig = signature(cell.keys);
            let sides: BTreeSet<Cell> = sig.iter().filter(|k| k.full_side()).filter_map(FaceKey::boundary_dir).collect();
            let full_cube = sides.len() == 6;
            lib.add(Entry { state: state.clone(), sig, overhang, tint, default_distance, full_cube });
        }
        lib.stats.states = lib.entries.len();
        Ok(lib)
    }

    fn add(&mut self, e: Entry) {
        if e.sig.is_empty() {
            return; // only overhanging geometry: never matchable from its own cell
        }
        let id = self.entries.len();
        self.exact.entry(e.sig.clone()).or_default().push(id);
        self.exact_norm.entry(normalized(&e.sig)).or_default().push(id);
        let textures: BTreeSet<&Arc<str>> = e.sig.iter().map(|k| &k.texture).collect();
        for t in textures {
            self.by_texture.entry(t.clone()).or_default().push(id);
        }
        self.entries.push(e);
    }

    pub fn exact(&self, sig: &[FaceKey]) -> Option<&[usize]> {
        self.exact.get(sig).map(Vec::as_slice)
    }

    pub fn exact_normalized(&self, norm: &[FaceKey]) -> Option<&[usize]> {
        self.exact_norm.get(norm).map(Vec::as_slice)
    }

    /// Entries whose signature uses every texture in `textures` (ids ascending).
    pub fn with_textures<'a>(&self, textures: impl IntoIterator<Item = &'a Arc<str>>) -> Vec<usize> {
        let mut lists: Vec<&Vec<usize>> = Vec::new();
        for t in textures {
            match self.by_texture.get(t) {
                Some(l) => lists.push(l),
                None => return Vec::new(),
            }
        }
        lists.sort_by_key(|l| l.len());
        let Some((first, rest)) = lists.split_first() else { return Vec::new() };
        first.iter().copied().filter(|id| rest.iter().all(|l| l.binary_search(id).is_ok())).collect()
    }
}

/// Debug-world block a face belongs to. An owner cell on an odd coordinate is the block itself; an even
/// one lies between two blocks (overhanging geometry), so pick the side whose block centre is nearer.
fn anchor(f: &WorldFace) -> Cell {
    let (ox, _, oz) = f.owner();
    let c = f.centroid();
    (axis_anchor(ox, c[0]), DEBUG_Y, axis_anchor(oz, c[2]))
}

fn axis_anchor(owner: i32, centroid: f32) -> i32 {
    if owner.rem_euclid(2) == 1 {
        return owner;
    }
    let (lo, hi) = (owner - 1, owner + 1);
    if (centroid - (lo as f32 + 0.5)).abs() <= (centroid - (hi as f32 + 0.5)).abs() { lo } else { hi }
}
