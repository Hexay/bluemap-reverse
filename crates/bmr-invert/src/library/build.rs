//! Learns a [`Library`] from BlueMap's render of the debug world.

use std::time::Instant;

use anyhow::{Context, Result};
use bmr_fetch::LocalMap;
use bmr_world::coords::{chunk_of, local};
use bmr_world::{BlockRegistry, BlockState, World};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use super::{Entry, Library, default_distance, is_full_cube};
use crate::face::{Cell, CellFaces, FaceKey, WorldFace, signature_uv, texture_ids, world_faces};
use crate::timings::Timings;

const DEBUG_Y: i32 = 70;

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
                    by_anchor.entry(a).or_default().push(f, a);
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
                    let Some(chunk) = chunks.get(&chunk_of(x, z)) else { continue };
                    if let Some(s) = chunk.block(local(x), y, local(z)) {
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
            let default_distance = registry.get(&state.name).map_or(0, |b| default_distance(state, b));
            let tint = cell.tint();
            let liquid = cell.keys.iter().find_map(FaceKey::liquid);
            let light = cell.light();
            let (sig, uvs) = signature_uv(cell.uvs);
            let full_cube = is_full_cube(&sig);
            lib.add(Entry {
                state: state.clone(),
                sig,
                uvs,
                light,
                liquid,
                overhang,
                tint,
                default_distance,
                full_cube,
            });
        }
        lib.stats.states = lib.entries.len();
        t.record("index", index_start.elapsed());
        lib.stats.timings = t;
        Ok(lib)
    }
}

/// Debug-world block a face belongs to. An owner cell on an odd coordinate is the block itself; an even
/// one lies between two blocks (overhanging geometry), so pick the side whose block centre is nearer.
fn anchor(f: &WorldFace) -> Cell {
    let (ox, _, oz) = f.owner();
    let c = f.centroid();
    (axis_anchor(ox, c[0]), DEBUG_Y, axis_anchor(oz, c[2]))
}

pub(super) fn axis_anchor(owner: i32, centroid: f32) -> i32 {
    if owner.rem_euclid(2) == 1 {
        return owner;
    }
    let (lo, hi) = (owner - 1, owner + 1);
    if (centroid - (lo as f32 + 0.5)).abs() <= (centroid - (hi as f32 + 0.5)).abs() { lo } else { hi }
}
