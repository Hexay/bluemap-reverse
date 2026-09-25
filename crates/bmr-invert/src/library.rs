//! Face-signature library learned from BlueMap's render of the vanilla debug world
//! (every state once, isolated, at y=70 on odd x/z). See docs/plan.md "Inversion by learned signatures".

use std::collections::BTreeSet;

use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::time::Instant;

use anyhow::{Context, Result};
use bmr_fetch::LocalMap;
use bmr_world::{BlockRegistry, BlockState, World};

use crate::face::{Cell, CellFaces, FaceKey, Liquid, Tex, WorldFace, normalized, signature, texture_ids, world_faces};
use crate::timings::Timings;

const DEBUG_Y: i32 = 70;

pub struct Entry {
    pub state: BlockState,
    /// In-cell, non-liquid faces (liquid faces depend on neighbours; handled by direction, not by key).
    pub sig: Vec<FaceKey>,
    /// Renders liquid: water/lava themselves, waterlogged=true, always-waterlogged plants.
    pub liquid: Option<Liquid>,
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
    pub timings: Timings,
}

pub struct Library {
    pub entries: Vec<Entry>,
    pub data_version: i32,
    pub stats: BuildStats,
    exact: FxHashMap<Vec<FaceKey>, Vec<usize>>,
    exact_norm: FxHashMap<Vec<FaceKey>, Vec<usize>>,
    by_texture: FxHashMap<Tex, Vec<usize>>,
    by_state: FxHashMap<BlockState, usize>,
}

impl Library {
    pub fn build(map: &LocalMap, world: &World, registry: &BlockRegistry) -> Result<Self> {
        let mut t = Timings::default();
        let faces = t.time("tiles", || -> Result<Vec<WorldFace>> {
            let names = texture_ids(&bmr_prbm::parse_texture_names(&map.textures_json()?)?);
            let per_tile = map
                .tiles(0)
                .par_iter()
                .map(|&tile| -> Result<Vec<WorldFace>> {
                    let parsed = bmr_prbm::parse(&map.tile_bytes(0, tile)?)?;
                    Ok(world_faces(&parsed, map.hires_origin(tile), &names))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(per_tile.into_iter().flatten().collect())
        })?;
        let (mut by_anchor, mut overhang) = t.time("attribute", || {
            let mut by_anchor: FxHashMap<Cell, CellFaces> = FxHashMap::default();
            let mut overhang: FxHashMap<Cell, Vec<(Cell, FaceKey)>> = FxHashMap::default();
            for f in &faces {
                let (a, o) = (anchor(f), f.owner());
                if o == a {
                    by_anchor.entry(a).or_default().push(f, f.key_at(a));
                } else {
                    overhang.entry(a).or_default().push(((o.0 - a.0, o.1 - a.1, o.2 - a.2), f.key_at(o)));
                }
            }
            (by_anchor, overhang)
        });
        // states whose every face overhangs still need an anchor entry
        for a in overhang.keys() {
            by_anchor.entry(*a).or_default();
        }

        // one region at a time, keeping only the anchor states: the decoded debug world is ~0.5 GB
        let (states, data_version) = t.time("world", || -> Result<(FxHashMap<Cell, BlockState>, i32)> {
            let mut states = FxHashMap::default();
            let mut data_version = None;
            for r in world.regions()? {
                let chunks = world.read_region(r)?;
                data_version = data_version.or_else(|| chunks.values().next().map(|c| c.data_version));
                for &(x, y, z) in by_anchor.keys() {
                    let Some(chunk) = chunks.get(&(x.div_euclid(16), z.div_euclid(16))) else { continue };
                    if let Some(s) = chunk.block(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize) {
                        states.insert((x, y, z), s.clone());
                    }
                }
            }
            Ok((states, data_version.context("debug world has no chunks")?))
        })?;
        let index_start = Instant::now();

        let mut lib = Self::empty(data_version);
        let mut anchors: Vec<_> = by_anchor.into_iter().collect();
        anchors.sort_by_key(|(c, _)| *c);
        for ((x, y, z), cell) in anchors {
            let Some(state) = states.get(&(x, y, z)) else { continue };
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
            let liquid = cell.keys.iter().find_map(FaceKey::liquid);
            let sig = signature(cell.keys.into_iter().filter(|k| k.liquid().is_none()).collect());
            let sides: BTreeSet<Cell> = sig.iter().filter(|k| k.full_side()).filter_map(FaceKey::boundary_dir).collect();
            let full_cube = sides.len() == 6;
            lib.add(Entry { state: state.clone(), sig, liquid, overhang, tint, default_distance, full_cube });
        }
        lib.stats.states = lib.entries.len();
        t.record("index", index_start.elapsed());
        lib.stats.timings = t;
        Ok(lib)
    }

    fn empty(data_version: i32) -> Self {
        Self {
            entries: Vec::new(),
            data_version,
            stats: BuildStats::default(),
            exact: FxHashMap::default(),
            exact_norm: FxHashMap::default(),
            by_texture: FxHashMap::default(),
            by_state: FxHashMap::default(),
        }
    }

    /// Rebuild a library from stored entries (bmr pack). Signatures are re-sorted: texture ids, and so
    /// key order, are process-local.
    pub fn from_entries(entries: Vec<Entry>, data_version: i32) -> Self {
        let mut lib = Self::empty(data_version);
        for mut e in entries {
            e.sig = signature(std::mem::take(&mut e.sig));
            lib.stats.overhang_faces += e.overhang.len();
            lib.stats.overhang_states += (!e.overhang.is_empty()) as usize;
            lib.add(e);
        }
        lib.stats.states = lib.entries.len();
        lib
    }

    fn add(&mut self, e: Entry) {
        if e.sig.is_empty() {
            return; // only overhanging geometry: never matchable from its own cell
        }
        let id = self.entries.len();
        self.exact.entry(e.sig.clone()).or_default().push(id);
        self.exact_norm.entry(normalized(&e.sig)).or_default().push(id);
        let textures: BTreeSet<Tex> = e.sig.iter().map(|k| k.texture).collect();
        for t in textures {
            self.by_texture.entry(t).or_default().push(id);
        }
        self.by_state.insert(e.state.clone(), id);
        self.entries.push(e);
    }

    pub fn find(&self, state: &BlockState) -> Option<usize> {
        self.by_state.get(state).copied()
    }

    /// The same state with `waterlogged=true`, if the block has that property.
    pub fn waterlogged_variant(&self, id: usize) -> Option<usize> {
        let s = &self.entries[id].state;
        let (i, _) = s.properties.iter().enumerate().find(|(_, (k, v))| k == "waterlogged" && v == "false")?;
        let mut w = s.clone();
        w.properties[i].1 = "true".into();
        self.find(&w)
    }

    pub fn exact(&self, sig: &[FaceKey]) -> Option<&[usize]> {
        self.exact.get(sig).map(Vec::as_slice)
    }

    pub fn exact_normalized(&self, norm: &[FaceKey]) -> Option<&[usize]> {
        self.exact_norm.get(norm).map(Vec::as_slice)
    }

    /// Entries whose signature uses every texture in `textures` (ids ascending).
    pub fn with_textures(&self, textures: impl IntoIterator<Item = Tex>) -> Vec<usize> {
        let mut lists: Vec<&Vec<usize>> = Vec::new();
        for t in textures {
            match self.by_texture.get(&t) {
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
