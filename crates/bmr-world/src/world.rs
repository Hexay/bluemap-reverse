//! World root + dimension → region directory, for both the 26.1+ layout and the legacy one.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::chunk::{Chunk, PaletteStyle};
use crate::nbt::decode_chunk;
use crate::region::read_region_where;
use crate::registry::BlockRegistry;

pub type ChunkPos = (i32, i32);

pub struct World {
    pub region_dir: PathBuf,
    /// Needed to expand 26.3 default-state palette shorthand (see nbt.rs).
    registry: Option<Arc<BlockRegistry>>,
}

/// Region folder of `dimension` (e.g. `minecraft:overworld`) under a world root: 26.1+ moved dimensions to
/// `dimensions/<ns>/<name>/region`; before that the overworld used `region/`, nether `DIM-1/`, end `DIM1/`.
pub fn region_dir(root: &Path, dimension: &str, modern: bool) -> PathBuf {
    let (ns, name) = dimension.split_once(':').unwrap_or(("minecraft", dimension));
    if modern {
        return root.join("dimensions").join(ns).join(name).join("region");
    }
    match (ns, name) {
        ("minecraft", "overworld") => root.join("region"),
        ("minecraft", "the_nether") => root.join("DIM-1").join("region"),
        ("minecraft", "the_end") => root.join("DIM1").join("region"),
        _ => root.join("dimensions").join(ns).join(name).join("region"),
    }
}

impl World {
    /// Tries the 26.1+ layout, then the legacy one (see `region_dir`).
    pub fn open(root: &Path, dimension: &str, registry: Option<Arc<BlockRegistry>>) -> Result<Self> {
        for dir in [region_dir(root, dimension, true), region_dir(root, dimension, false)] {
            if dir.is_dir() {
                return Ok(Self { region_dir: dir, registry });
            }
        }
        bail!("no region dir for {dimension} under {}", root.display())
    }

    /// World without any regions yet (reads as all air).
    pub fn empty(region_dir: PathBuf) -> Self {
        Self { region_dir, registry: None }
    }

    /// Palette encoding of this world's chunks (from the first chunk with a named palette entry).
    pub fn palette_style(&self) -> Result<Option<PaletteStyle>> {
        for r in self.regions()? {
            if let Some(s) = self.read_region(r)?.values().find_map(|c| c.palette_style) {
                return Ok(Some(s));
            }
        }
        Ok(None)
    }

    /// Region coordinates of every `r.<x>.<z>.mca` present.
    pub fn regions(&self) -> Result<Vec<(i32, i32)>> {
        let Ok(entries) = fs::read_dir(&self.region_dir) else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for e in entries {
            let name = e?.file_name();
            let parts: Vec<&str> = name.to_str().unwrap_or("").split('.').collect();
            if let ["r", x, z, "mca"] = parts.as_slice() {
                if let (Ok(x), Ok(z)) = (x.parse(), z.parse()) {
                    out.push((x, z));
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// Every chunk in region (rx, rz); empty if the file is absent.
    pub fn read_region(&self, region: (i32, i32)) -> Result<FxHashMap<ChunkPos, Chunk>> {
        self.read_region_where(region, &|_| true)
    }

    /// Only chunks for which `keep(chunk pos)` holds are decompressed and decoded.
    pub fn read_region_where(
        &self,
        (rx, rz): (i32, i32),
        keep: &(dyn Fn(ChunkPos) -> bool + Sync),
    ) -> Result<FxHashMap<ChunkPos, Chunk>> {
        let path = self.region_dir.join(format!("r.{rx}.{rz}.mca"));
        if !path.exists() {
            return Ok(FxHashMap::default());
        }
        let keep_local = |(lx, lz): (u8, u8)| keep((rx * 32 + lx as i32, rz * 32 + lz as i32));
        read_region_where(&path, &keep_local)?
            .into_par_iter()
            .map(|((lx, lz), nbt)| {
                let pos = (rx * 32 + lx as i32, rz * 32 + lz as i32);
                let chunk = decode_chunk(&nbt, self.registry.as_deref())
                    .with_context(|| format!("chunk {pos:?} in {}", path.display()))?;
                Ok((pos, chunk))
            })
            .collect()
    }
}
