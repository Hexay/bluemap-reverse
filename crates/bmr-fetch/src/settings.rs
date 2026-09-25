//! Webapp `settings.json` and per-map `maps/<id>/settings.json` (research/01 §1).

use serde::Deserialize;

use crate::grid::Grid;

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
        (self.start_pos[0].floor() as i32, self.start_pos[1].floor() as i32)
    }
}
