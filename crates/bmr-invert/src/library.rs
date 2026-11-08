//! Face-signature library learned from BlueMap's render of the vanilla debug world
//! (every state once, isolated, at y=70 on odd x/z). See docs/architecture.md "Inversion by learned signatures".

mod build;

use std::collections::BTreeSet;

use bmr_world::{BlockInfo, BlockState};
use rustc_hash::FxHashMap;

use crate::face::{Cell, FaceKey, Liquid, Tex, Uv, normalized, signature_uv};
use crate::timings::Timings;

#[derive(Debug, PartialEq)]
pub struct Entry {
    pub state: BlockState,
    /// In-cell, non-liquid faces (liquid faces depend on neighbours; handled by direction, not by key).
    pub sig: Vec<FaceKey>,
    /// UVs of `sig`, index for index (ties among equal keys sorted by UV).
    pub uvs: Vec<Uv>,
    /// Dimmest block light on its faces in the debug world (≈ its own emission − 1; see `CellFaces`).
    pub light: u8,
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
    /// Face key (relative to the cell it lands in) → offsets from the cells that may overhang it there.
    overhang_from: FxHashMap<FaceKey, Vec<Cell>>,
}

impl Library {
    fn empty(data_version: i32) -> Self {
        Self {
            entries: Vec::new(),
            data_version,
            stats: BuildStats::default(),
            exact: FxHashMap::default(),
            exact_norm: FxHashMap::default(),
            by_texture: FxHashMap::default(),
            by_state: FxHashMap::default(),
            overhang_from: FxHashMap::default(),
        }
    }

    /// Rebuild a library from stored entries (bmr pack). Signatures are re-sorted: texture ids, and so
    /// key order, are process-local.
    pub fn from_entries(entries: Vec<Entry>, data_version: i32) -> Self {
        let mut lib = Self::empty(data_version);
        for mut e in entries {
            (e.sig, e.uvs) =
                signature_uv(std::mem::take(&mut e.sig).into_iter().zip(std::mem::take(&mut e.uvs)).collect());
            lib.stats.overhang_faces += e.overhang.len();
            lib.stats.overhang_states += (!e.overhang.is_empty()) as usize;
            lib.add(e);
        }
        lib.stats.states = lib.entries.len();
        lib
    }

    fn add(&mut self, e: Entry) {
        for &(off, key) in &e.overhang {
            let from = self.overhang_from.entry(key).or_default();
            if !from.contains(&off) {
                from.push(off);
            }
        }
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

    /// An entry drawing the same solid faces as `id` but holding `liquid` (liquid faces are matched apart):
    /// the same state with `waterlogged` flipped, else any same-signature state (cauldron ↔ lava_cauldron).
    /// None when `id` already fits or nothing does.
    pub fn liquid_variant(&self, id: usize, liquid: Option<Liquid>) -> Option<usize> {
        let e = &self.entries[id];
        if e.liquid == liquid {
            return None;
        }
        let (from, to) = if liquid == Some(Liquid::Water) { ("false", "true") } else { ("true", "false") };
        if let Some(i) = e.state.properties.iter().position(|(k, v)| k == "waterlogged" && v == from) {
            let mut w = e.state.clone();
            w.properties[i].1 = to.into();
            if let Some(w) = self.find(&w).filter(|&w| self.entries[w].liquid == liquid) {
                return Some(w);
            }
        }
        self.exact(&e.sig)?
            .iter()
            .copied()
            .filter(|&c| self.entries[c].liquid == liquid)
            .min_by_key(|&c| (self.entries[c].default_distance, c))
    }

    /// Offsets (source → this cell) of the neighbours whose block may have drawn `key` into this cell.
    pub fn overhang_from(&self, key: &FaceKey) -> &[Cell] {
        self.overhang_from.get(key).map_or(&[], Vec::as_slice)
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

/// Number of properties differing from the block's default state.
pub fn default_distance(state: &BlockState, info: &BlockInfo) -> u32 {
    state.properties.iter().filter(|p| !info.default.contains(p)).count() as u32
}

/// Renders all 6 sides as full outer quads.
pub fn is_full_cube(sig: &[FaceKey]) -> bool {
    sig.iter().filter(|k| k.full_side()).filter_map(FaceKey::boundary_dir).collect::<BTreeSet<Cell>>().len() == 6
}

#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
