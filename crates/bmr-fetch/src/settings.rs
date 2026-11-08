//! Webapp `settings.json` and per-map `maps/<id>/settings.json` (docs/research/01 §1). Both come from the site,
//! so `parse` rejects values that would escape the mirror directory or break the tile grid arithmetic.

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::grid::Grid;

/// Tile sizes, translations and lowres tile spans stay below this, so grid math on world coordinates fits i32.
const MAX_GRID: i32 = 1 << 24;
const MAX_LOD_COUNT: u32 = 16;
/// Minecraft's world border.
const MAX_COORD: f64 = 30_000_000.0;

impl SiteSettings {
    pub fn parse(json: &[u8]) -> Result<Self> {
        let s: Self = serde_json::from_slice(json).context("settings.json")?;
        ensure!(is_relative_path(&s.map_data_root), "settings.json: bad mapDataRoot {:?}", s.map_data_root);
        for id in &s.maps {
            ensure!(is_relative_path(id) && !id.contains('/'), "settings.json: bad map id {id:?}");
        }
        Ok(s)
    }
}

/// Map ids and the data root become directories under the mirror: no `..`, no root, no drive or `\`.
fn is_relative_path(p: &str) -> bool {
    p.split('/')
        .all(|seg| !seg.is_empty() && seg != ".." && !seg.chars().any(|c| c == '\\' || c == ':' || c.is_control()))
}

impl MapSettings {
    pub fn parse(json: &[u8]) -> Result<Self> {
        let s: Self = serde_json::from_slice(json)?;
        let in_grid = |v: i32| (1..=MAX_GRID).contains(&v);
        ensure!(s.hires.tile_size.into_iter().all(in_grid), "bad hires tileSize {:?}", s.hires.tile_size);
        ensure!(s.hires.translate.iter().all(|t| t.abs() <= MAX_GRID), "bad translate {:?}", s.hires.translate);
        let l = &s.lowres;
        ensure!(l.lod_factor >= 1 && l.lod_count <= MAX_LOD_COUNT, "bad lod {}/{}", l.lod_factor, l.lod_count);
        let span = l.lod_factor.checked_pow(l.lod_count.saturating_sub(1)).unwrap_or(i32::MAX);
        let fits = l.tile_size.iter().all(|&w| in_grid(w) && w.checked_mul(span).is_some_and(in_grid));
        ensure!(fits, "bad lowres tileSize {:?} at lodFactor {}", l.tile_size, l.lod_factor);
        Ok(s)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSettings {
    /// BlueMap version that generated the webapp (e.g. "5.27").
    #[serde(default)]
    pub version: Option<String>,
    pub maps: Vec<String>,
    #[serde(default = "default_maps_root")]
    pub map_data_root: String,
}

fn default_maps_root() -> String {
    "maps".into()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapSettings {
    pub name: String,
    pub hires: HiresSettings,
    pub lowres: LowresSettings,
    #[serde(default)]
    pub start_pos: [f64; 2],
    #[serde(default)]
    pub ambient_light: f32,
    /// RGBA 0..1; BlueMap's per-dimension defaults tell the dimension apart when the map id does not.
    #[serde(default)]
    pub sky_color: Option<[f32; 4]>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiresSettings {
    pub tile_size: [i32; 2],
    pub translate: [i32; 2],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LowresSettings {
    pub tile_size: [i32; 2],
    pub lod_factor: i32,
    pub lod_count: u32,
}

impl MapSettings {
    pub fn hires_grid(&self) -> Grid {
        Grid { size: self.hires.tile_size, offset: self.hires.translate }
    }

    /// Blocks per lowres pixel at `lod` (1-based).
    pub fn lod_scale(&self, lod: u32) -> i32 {
        self.lowres.lod_factor.pow(lod - 1)
    }

    pub fn lowres_grid(&self, lod: u32) -> Grid {
        let s = self.lod_scale(lod);
        let [w, h] = self.lowres.tile_size;
        Grid { size: [w * s, h * s], offset: [0, 0] }
    }

    pub fn start_block(&self) -> (i32, i32) {
        let c = |v: f64| v.floor().clamp(-MAX_COORD, MAX_COORD) as i32;
        (c(self.start_pos[0]), c(self.start_pos[1]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(root: &str, id: &str) -> Result<SiteSettings> {
        SiteSettings::parse(&serde_json::to_vec(&serde_json::json!({"maps": [id], "mapDataRoot": root})).unwrap())
    }

    #[test]
    fn site_paths_must_stay_inside_the_mirror() {
        site("maps", "world").unwrap();
        site("./data/maps", "the_end").unwrap();
        for (root, id) in [("..", "w"), ("/etc", "w"), ("maps", "../x"), ("maps", "a/b"), ("C:", "w"), ("maps", "a\\b")]
        {
            assert!(site(root, id).is_err(), "{root} {id}");
        }
    }

    fn map(hires: [i32; 2], lowres: [i32; 2], factor: i32, count: u32, start: f64) -> Result<MapSettings> {
        MapSettings::parse(
            &serde_json::to_vec(&serde_json::json!({"name": "m", "startPos": [start, start],
                "hires": {"tileSize": hires, "translate": [2, 2]},
                "lowres": {"tileSize": lowres, "lodFactor": factor, "lodCount": count}}))
            .unwrap(),
        )
    }

    #[test]
    fn bluemap_defaults_parse() {
        let m = map([32, 32], [500, 500], 5, 3, 0.0).unwrap();
        assert_eq!(m.lowres_grid(3).size, [12500, 12500]);
    }

    #[test]
    fn grids_that_break_tile_math_are_refused() {
        // zero sizes divided by zero in tile_of; huge factors overflowed lod_scale; huge counts looped for ever
        for bad in [
            map([0, 32], [500, 500], 5, 3, 0.0),
            map([32, 32], [500, -1], 5, 3, 0.0),
            map([32, 32], [500, 500], 0, 3, 0.0),
            map([32, 32], [500, 500], 1000, 4, 0.0),
            map([32, 32], [500, 500], 1, 4_000_000_000, 0.0),
        ] {
            assert!(bad.is_err());
        }
    }

    #[test]
    fn start_block_is_clamped_to_the_world() {
        let m = map([32, 32], [500, 500], 5, 3, 1e300).unwrap();
        assert_eq!(m.start_block(), (30_000_000, 30_000_000));
        m.hires_grid().tiles_in(m.start_block().0, 0, 12500, 1).count();
    }
}
