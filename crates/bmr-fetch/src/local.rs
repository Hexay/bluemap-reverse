//! Read side of a mirror written by `mirror()`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::grid::{Tile, tile_file};
use crate::settings::{MapSettings, SiteSettings};
use crate::store::{Manifest, manifest_rel};

pub struct LocalMap {
    pub id: String,
    pub settings: MapSettings,
    pub manifest: Manifest,
    dir: PathBuf,
}

impl LocalMap {
    /// `id` may be omitted when the mirror holds exactly one map.
    pub fn open(mirror: &Path, id: Option<&str>) -> Result<Self> {
        let site: SiteSettings = serde_json::from_slice(&read(&mirror.join("settings.json"))?)?;
        let id = match (id, site.maps.as_slice()) {
            (Some(id), _) => id.to_owned(),
            (None, [only]) => only.clone(),
            (None, maps) => bail!("mirror has maps {maps:?}; pick one"),
        };
        let dir = mirror.join(&site.map_data_root).join(&id);
        let settings = serde_json::from_slice(&read(&dir.join("settings.json"))?)
            .with_context(|| format!("{id}/settings.json"))?;
        let manifest = Manifest::load(&mirror.join(manifest_rel(&id)))?;
        Ok(Self { id, settings, manifest, dir })
    }

    /// Tiles that exist at `lod` (0 = hires), per the fetch manifest.
    pub fn tiles(&self, lod: u32) -> Vec<Tile> {
        self.manifest.layers.get(&lod).map(|p| p.present.iter().copied().collect()).unwrap_or_default()
    }

    pub fn tile_bytes(&self, lod: u32, t: Tile) -> Result<Vec<u8>> {
        read(&self.dir.join(tile_file(lod, t)))
    }

    pub fn textures_json(&self) -> Result<Vec<u8>> {
        read(&self.dir.join("textures.json"))
    }

    /// World x/z of a hires tile's min corner.
    pub fn hires_origin(&self, t: Tile) -> [i32; 2] {
        let (x, z) = self.settings.hires_grid().tile_min(t);
        [x, z]
    }
}

fn read(p: &Path) -> Result<Vec<u8>> {
    fs::read(p).with_context(|| p.display().to_string())
}
