//! Window planning for bounded-memory reversal: one output region (512×512 columns) at a time, loaded
//! with a halo of hires tiles so every cross-cell rule (overhang ±1, evidence ±1, leaves ≤7, prior
//! searches ≤32) sees complete neighbourhoods. Halo cells are computed but only the region is emitted.
//! An `--area` clips the emitted columns the same way: its border gets the same halo.

use std::collections::BTreeSet;

use bmr_fetch::grid::Tile;
use bmr_fetch::{BlockBox, LocalMap};
use bmr_world::ChunkPos;
use bmr_world::coords::{chunk_of, region_of};

pub type Region = (i32, i32);

pub struct Window {
    /// `None` = the whole map in one pass (no region filter).
    pub region: Option<Region>,
    /// Only columns inside are emitted.
    pub area: Option<BlockBox>,
    /// Rendered hires tiles overlapping the emitted columns + halo.
    pub tiles: Vec<Tile>,
    /// Rendered columns that are emitted.
    pub columns: Vec<(i32, i32)>,
    /// Rendered columns in emitted + halo: the prior fill's neighbour searches (same-height copy, deep-water
    /// floors) must see resolved columns beyond the border to match a whole-map run.
    pub halo_columns: Vec<(i32, i32)>,
    /// Chunk box of emitted + halo, for loading the regeneration; `None` = everything.
    pub chunk_area: Option<(ChunkPos, ChunkPos)>,
}

impl Window {
    pub fn emits(&self, x: i32, z: i32) -> bool {
        self.region.is_none_or(|r| region_of(x, z) == r) && self.area.is_none_or(|a| a.contains(x, z))
    }
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

/// One window over the map, or over `area` + `halo` blocks.
pub fn whole_map(map: &LocalMap, area: Option<BlockBox>, halo: i32) -> Vec<Window> {
    let columns = rendered_columns(map);
    match area {
        Some(a) => vec![window(map, &columns, None, area, a.grow(halo))],
        None => vec![Window {
            region: None,
            area: None,
            tiles: map.tiles(0),
            halo_columns: columns.clone(),
            columns,
            chunk_area: None,
        }],
    }
}

/// One window per region that has rendered columns (inside `area`, if given). `halo` in blocks.
pub fn per_region(map: &LocalMap, halo: i32, area: Option<BlockBox>) -> Vec<Window> {
    let all_columns = rendered_columns(map);
    let regions: BTreeSet<Region> = all_columns
        .iter()
        .filter(|&&(x, z)| area.is_none_or(|a| a.contains(x, z)))
        .map(|&(x, z)| region_of(x, z))
        .collect();
    regions
        .into_iter()
        .filter_map(|(rx, rz)| {
            let reach = BlockBox::new(rx * 512, rz * 512, rx * 512 + 511, rz * 512 + 511).grow(halo);
            let reach = match area {
                Some(a) => reach.intersect(a.grow(halo))?,
                None => reach,
            };
            Some(window(map, &all_columns, Some((rx, rz)), area, reach))
        })
        .collect()
}

/// Window loading everything within `reach` and emitting what `region` and `area` allow.
fn window(
    map: &LocalMap,
    all_columns: &[(i32, i32)],
    region: Option<Region>,
    area: Option<BlockBox>,
    reach: BlockBox,
) -> Window {
    let grid = map.settings.hires_grid();
    let tiles = map.tiles(0).into_iter().filter(|&t| reach.touches(&grid, t)).collect();
    let halo_columns: Vec<_> = all_columns.iter().copied().filter(|&(x, z)| reach.contains(x, z)).collect();
    let chunk_area = (chunk_of(reach.min[0], reach.min[1]), chunk_of(reach.max[0], reach.max[1]));
    let mut w = Window { region, area, tiles, columns: Vec::new(), halo_columns, chunk_area: Some(chunk_area) };
    w.columns = w.halo_columns.iter().copied().filter(|&(x, z)| w.emits(x, z)).collect();
    w
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Mirror with hires tiles x -20..=20 on row z 0 (32-block tiles offset by 2: x -638..=673, z 2..=33).
    fn strip_map(test: &str) -> LocalMap {
        let dir = std::env::temp_dir().join(format!("bmr-window-{test}-{}", std::process::id()));
        let maps = dir.join("maps/m");
        fs::create_dir_all(&maps).unwrap();
        fs::create_dir_all(dir.join("bmr-manifest")).unwrap();
        fs::write(dir.join("settings.json"), r#"{"maps":["m"]}"#).unwrap();
        let settings = r#"{"name":"m","hires":{"tileSize":[32,32],"translate":[2,2]},
            "lowres":{"tileSize":[500,500],"lodFactor":5,"lodCount":3}}"#;
        fs::write(maps.join("settings.json"), settings).unwrap();
        let present: Vec<[i32; 2]> = (-20..=20).map(|x| [x, 0]).collect();
        let manifest = serde_json::json!({"layers": {"0": {"present": present, "empty": []}}});
        fs::write(dir.join("bmr-manifest/m.json"), manifest.to_string()).unwrap();
        let map = LocalMap::open(&dir, None).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        map
    }

    #[test]
    fn area_clips_emitted_columns_and_keeps_the_halo() {
        let map = strip_map("area");
        let area = BlockBox::new(500, 10, 530, 12);
        let windows = per_region(&map, 32, Some(area));
        assert_eq!(windows.iter().map(|w| w.region).collect::<Vec<_>>(), [Some((0, 0)), Some((1, 0))]);
        let emitted: Vec<_> = windows.iter().flat_map(|w| w.columns.iter().copied()).collect();
        assert_eq!(emitted.len(), 31 * 3);
        assert!(emitted.iter().all(|&(x, z)| area.contains(x, z)));
        let w0 = &windows[0];
        // reach x 468..=543 (area + halo, cut at the region's own halo) = tiles 14..=16
        assert_eq!(w0.tiles, [(14, 0), (15, 0), (16, 0)]);
        assert!(w0.halo_columns.contains(&(468, 2)) && !w0.halo_columns.contains(&(467, 2)));
        assert!(!w0.emits(512, 10) && w0.emits(511, 10) && !w0.emits(511, 13));
        assert_eq!(whole_map(&map, Some(area), 32)[0].columns.len(), 31 * 3);
    }

    #[test]
    fn no_area_windows_cover_every_column() {
        let map = strip_map("whole");
        let windows = per_region(&map, 32, None);
        assert_eq!(windows.len(), 4);
        assert_eq!(windows.iter().map(|w| w.columns.len()).sum::<usize>(), 41 * 32 * 32);
        assert_eq!(whole_map(&map, None, 32)[0].chunk_area, None);
    }
}
