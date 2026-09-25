//! Output world = template world (level.dat, data/, …) minus its chunk data, plus our region files.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use rayon::prelude::*;

use crate::chunk::Chunk;
use crate::nbt_write::encode_chunk;
use crate::region::write_region;
use crate::registry::BlockRegistry;

/// Per-dimension dirs holding chunk-derived data that must not leak from the template.
const CHUNK_DIRS: [&str; 3] = ["region", "entities", "poi"];

pub struct WorldWriter {
    region_dir: PathBuf,
    registry: Arc<BlockRegistry>,
}

impl WorldWriter {
    /// Creates `out` from `template` (26.1+ layout). Refuses to overwrite an existing world.
    pub fn create(out: &Path, template: &Path, dimension: &str, registry: Arc<BlockRegistry>) -> Result<Self> {
        if out.join("level.dat").exists() {
            bail!("{} already contains a world", out.display());
        }
        ensure_template(template)?;
        copy_tree(template, out)?;
        let (ns, name) = dimension.split_once(':').unwrap_or(("minecraft", dimension));
        let region_dir = out.join("dimensions").join(ns).join(name).join("region");
        fs::create_dir_all(&region_dir)?;
        Ok(Self { region_dir, registry })
    }

    /// Writes every chunk, grouped into region files (one file per region, written once).
    pub fn write_chunks(&self, chunks: Vec<Chunk>) -> Result<usize> {
        let mut regions: BTreeMap<(i32, i32), Vec<Chunk>> = BTreeMap::new();
        for c in chunks {
            regions.entry((c.x.div_euclid(32), c.z.div_euclid(32))).or_default().push(c);
        }
        let count = regions.len();
        regions.into_par_iter().try_for_each(|((rx, rz), chunks)| -> Result<()> {
            // per chunk, not per region: a windowed run hands over one region at a time
            let encoded = chunks
                .par_iter()
                .map(|c| {
                    let nbt = encode_chunk(c, &self.registry).with_context(|| format!("chunk {},{}", c.x, c.z))?;
                    Ok(((c.x.rem_euclid(32) as u8, c.z.rem_euclid(32) as u8), nbt))
                })
                .collect::<Result<Vec<_>>>()?;
            write_region(&self.region_dir.join(format!("r.{rx}.{rz}.mca")), &encoded)
        })?;
        Ok(count)
    }
}

fn ensure_template(template: &Path) -> Result<()> {
    if !template.join("level.dat").exists() {
        bail!("template {} has no level.dat (py -3 tools/make_world.py template-void)", template.display());
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "session.lock" || CHUNK_DIRS.contains(&name.as_ref()) {
            continue;
        }
        let dst = to.join(&*name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &dst)?;
        } else {
            fs::copy(entry.path(), &dst).with_context(|| dst.display().to_string())?;
        }
    }
    Ok(())
}
