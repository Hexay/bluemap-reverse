//! Local mirror: files at their URL-relative paths (decompressed) + a per-map manifest of probed tiles.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::grid::Tile;

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    pub fn read(&self, rel: &str) -> Result<Vec<u8>> {
        let p = self.path(rel);
        fs::read(&p).with_context(|| p.display().to_string())
    }

    pub fn write(&self, rel: &str, bytes: &[u8]) -> Result<()> {
        write_atomic(&self.path(rel), bytes)
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("part");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).with_context(|| path.display().to_string())
}

pub fn manifest_rel(map_id: &str) -> String {
    format!("bmr-manifest/{map_id}.json")
}

/// Tiles already probed for one layer; `empty` = server answered 204/404. Never re-fetched.
#[derive(Default, Serialize, Deserialize)]
pub struct Probed {
    pub present: BTreeSet<Tile>,
    pub empty: BTreeSet<Tile>,
}

impl Probed {
    pub fn known(&self, t: &Tile) -> bool {
        self.present.contains(t) || self.empty.contains(t)
    }
}

/// Keyed by lod: 0 = hires, 1..=lodCount = lowres.
#[derive(Default, Serialize, Deserialize)]
pub struct Manifest {
    pub layers: BTreeMap<u32, Probed>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(b) => serde_json::from_slice(&b).with_context(|| path.display().to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        write_atomic(path, &serde_json::to_vec(self)?)
    }
}
