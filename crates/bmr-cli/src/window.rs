//! Window planning for bounded-memory reversal: one output region (512×512 columns) at a time, loaded
//! with a halo of hires tiles so every cross-cell rule (overhang ±1, evidence ±1, leaves ≤7, prior
//! searches ≤32) sees complete neighbourhoods. Halo cells are computed but only the region is emitted.

use std::collections::BTreeMap;

use bmr_fetch::LocalMap;
use bmr_fetch::grid::Tile;
use bmr_world::ChunkPos;

pub type Region = (i32, i32);

pub struct Window {
    /// `None` = the whole map in one pass (no emit filter).
    pub region: Option<Region>,
    /// Rendered hires tiles overlapping the region + halo.
    pub tiles: Vec<Tile>,
    /// Rendered columns inside the region (the ones emitted).
    pub columns: Vec<(i32, i32)>,
    /// Rendered columns in region + halo: the prior fill's neighbour searches (same-height copy, deep-water
    /// floors) must see resolved columns beyond the border to match a whole-map run.
    pub halo_columns: Vec<(i32, i32)>,
    /// Chunk box of region + halo, for loading the regeneration.
    pub chunk_area: Option<(ChunkPos, ChunkPos)>,
}

impl Window {
    pub fn emits(&self, x: i32, z: i32) -> bool {
        self.region.is_none_or(|r| region_of(x, z) == r)
    }
}

pub fn region_of(x: i32, z: i32) -> Region {
    (x.div_euclid(512), z.div_euclid(512))
}

/// Every world column inside a rendered hires tile.
pub fn rendered_columns(map: &LocalMap) -> Vec<(i32, i32)> {
    let [w, h] = map.settings.hires.tile_size;
    map.tiles(0)
        .into_iter()
        .flat_map(|t| {
            let [x0, z0] = map.hires_origin(t);
            (x0..x0 + w).flat_map(move |x| (z0..z0 + h).map(move |z| (x, z)))
        })
        .collect()
}

pub fn whole_map(map: &LocalMap) -> Vec<Window> {
    let columns = rendered_columns(map);
    vec![Window { region: None, tiles: map.tiles(0), halo_columns: columns.clone(), columns, chunk_area: None }]
}

/// One window per region that has rendered columns. `halo` in blocks.
pub fn per_region(map: &LocalMap, halo: i32) -> Vec<Window> {
    let all_columns = rendered_columns(map);
    let mut columns: BTreeMap<Region, Vec<(i32, i32)>> = BTreeMap::new();
    for &(x, z) in &all_columns {
        columns.entry(region_of(x, z)).or_default().push((x, z));
    }
    let [w, h] = map.settings.hires.tile_size;
    let all_tiles = map.tiles(0);
    columns
        .into_iter()
        .map(|((rx, rz), columns)| {
            let (x0, z0, x1, z1) = (rx * 512 - halo, rz * 512 - halo, rx * 512 + 511 + halo, rz * 512 + 511 + halo);
            let tiles = all_tiles
                .iter()
                .copied()
                .filter(|&t| {
                    let [tx, tz] = map.hires_origin(t);
                    tx <= x1 && tx + w - 1 >= x0 && tz <= z1 && tz + h - 1 >= z0
                })
                .collect();
            let chunk_area = ((x0.div_euclid(16), z0.div_euclid(16)), (x1.div_euclid(16), z1.div_euclid(16)));
            let halo_columns = all_columns
                .iter()
                .copied()
                .filter(|&(x, z)| (x0..=x1).contains(&x) && (z0..=z1).contains(&z))
                .collect();
            Window { region: Some((rx, rz)), tiles, columns, halo_columns, chunk_area: Some(chunk_area) }
        })
        .collect()
}
