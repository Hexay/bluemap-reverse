//! Mirror a BlueMap site: settings → per map: settings/textures/markers → lowres coarse→fine → hires.
//! Each finer layer is seeded from the visible pixels of the coarser one, then flood-filled.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use rayon::prelude::*;

use crate::discover::discover;
use crate::grid::{Grid, Tile, tile_file};
use crate::http::Http;
use crate::lowres::LowresImage;
use crate::progress::{OnProgress, Progress, emit};
use crate::settings::{MapSettings, SiteSettings};
use crate::store::{Manifest, Store, manifest_rel};

pub struct Options {
    pub base_url: String,
    pub out: PathBuf,
    /// Empty = every map listed in settings.json.
    pub maps: Vec<String>,
    pub concurrency: usize,
    pub delay: Duration,
    /// Receives retries and per-layer probing progress.
    pub progress: Option<OnProgress>,
}

#[derive(Debug)]
pub struct MapSummary {
    pub id: String,
    /// (lod, present, empty) per layer, lod 0 = hires.
    pub layers: Vec<(u32, usize, usize)>,
}

pub fn mirror(opts: &Options) -> Result<Vec<MapSummary>> {
    let http = Http::new(&opts.base_url, opts.delay)?.with_progress(opts.progress.clone());
    let store = Store::new(&opts.out);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(opts.concurrency).build()?;

    let site_bytes = http.get("settings.json")?.context("site has no settings.json")?;
    store.write("settings.json", &site_bytes)?;
    let site: SiteSettings = serde_json::from_slice(&site_bytes).context("settings.json")?;

    let ids: Vec<&String> =
        site.maps.iter().filter(|m| opts.maps.is_empty() || opts.maps.contains(m)).collect();
    pool.install(|| {
        ids.into_iter()
            .map(|id| MapMirror::new(&http, &store, &site.map_data_root, id, opts.progress.as_ref())?.run())
            .collect()
    })
}

struct MapMirror<'a> {
    http: &'a Http,
    store: &'a Store,
    id: String,
    root: String,
    settings: MapSettings,
    manifest_path: PathBuf,
    manifest: Manifest,
    progress: Option<&'a OnProgress>,
}

impl<'a> MapMirror<'a> {
    fn new(http: &'a Http, store: &'a Store, maps_root: &str, id: &str, progress: Option<&'a OnProgress>) -> Result<Self> {
        let root = format!("{maps_root}/{id}");
        let bytes = http.get(&format!("{root}/settings.json"))?.context("map settings.json missing")?;
        store.write(&format!("{root}/settings.json"), &bytes)?;
        let settings = serde_json::from_slice(&bytes).with_context(|| format!("{root}/settings.json"))?;
        let manifest_path = store.path(&manifest_rel(id));
        let manifest = Manifest::load(&manifest_path)?;
        Ok(Self { http, store, id: id.into(), root, settings, manifest_path, manifest, progress })
    }

    fn run(mut self) -> Result<MapSummary> {
        self.fetch_required("textures.json")?;
        self.fetch_optional("live/markers.json")?;

        let start: BTreeSet<Tile> = [self.settings.start_block(), (0, 0)].into();
        let lod_count = self.settings.lowres.lod_count;
        let mut seed_blocks = start.clone();
        let mut seed_scale = 1;
        for lod in (1..=lod_count).rev() {
            let grid = self.settings.lowres_grid(lod);
            let seeds = seeds_from(&grid, &seed_blocks, seed_scale);
            self.discover_layer(lod, seeds)?;
            seed_blocks = self.visible_blocks(lod)?;
            seed_scale = self.settings.lod_scale(lod);
        }
        let hires = self.settings.hires_grid();
        let mut seeds = seeds_from(&hires, &seed_blocks, seed_scale);
        seeds.extend(start.iter().map(|&(x, z)| hires.tile_of(x, z)));
        self.discover_layer(0, seeds)?;

        let layers = self.manifest.layers.iter().map(|(&l, p)| (l, p.present.len(), p.empty.len())).collect();
        Ok(MapSummary { id: self.id, layers })
    }

    fn fetch_required(&self, rel: &str) -> Result<()> {
        let rel = format!("{}/{rel}", self.root);
        let bytes = self.http.get(&rel)?.with_context(|| format!("{rel} missing"))?;
        self.store.write(&rel, &bytes)
    }

    fn fetch_optional(&self, rel: &str) -> Result<()> {
        let rel = format!("{}/{rel}", self.root);
        match self.http.get(&rel)? {
            Some(bytes) => self.store.write(&rel, &bytes),
            None => Ok(()),
        }
    }

    fn discover_layer(&mut self, lod: u32, seeds: BTreeSet<Tile>) -> Result<()> {
        let mut probed = self.manifest.layers.remove(&lod).unwrap_or_default();
        let (http, store, root) = (self.http, self.store, &self.root);
        let fetch = |t: Tile| -> Result<bool> {
            let rel = tile_rel(root, lod, t);
            match http.get(&rel)? {
                Some(bytes) => store.write(&rel, &bytes).map(|_| true),
                None => Ok(false),
            }
        };
        let (manifest, path, map, progress) = (&mut self.manifest, &self.manifest_path, self.id.as_str(), self.progress);
        let result = discover(&mut probed, seeds, fetch, |p| {
            emit(progress, Progress::Layer { map, lod, present: p.present.len(), empty: p.empty.len() });
            Ok(())
        });
        manifest.layers.insert(lod, probed);
        manifest.save(path)?;
        result
    }

    /// Min-corner block of every visible pixel in every present tile of lowres layer `lod`.
    fn visible_blocks(&self, lod: u32) -> Result<BTreeSet<Tile>> {
        let grid = self.settings.lowres_grid(lod);
        let scale = self.settings.lod_scale(lod);
        let present: Vec<Tile> = self.manifest.layers[&lod].present.iter().copied().collect();
        let per_tile: Vec<Vec<Tile>> = present
            .par_iter()
            .map(|&t| {
                let rel = tile_rel(&self.root, lod, t);
                let (x0, z0) = grid.tile_min(t);
                let px = LowresImage::decode(&self.store.read(&rel)?).with_context(|| rel)?.visible_pixels();
                Ok(px.into_iter().map(|(x, z)| (x0 + x * scale, z0 + z * scale)).collect())
            })
            .collect::<Result<_>>()?;
        Ok(per_tile.into_iter().flatten().collect())
    }
}

fn tile_rel(map_root: &str, lod: u32, t: Tile) -> String {
    format!("{map_root}/{}", tile_file(lod, t))
}

/// Tiles of `grid` overlapping any `scale`×`scale` block square whose min corner is in `blocks`.
fn seeds_from(grid: &Grid, blocks: &BTreeSet<Tile>, scale: i32) -> BTreeSet<Tile> {
    blocks.iter().flat_map(|&(x, z)| grid.tiles_in(x, z, scale, scale)).collect()
}
