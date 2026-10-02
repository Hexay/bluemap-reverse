//! Output world = template world (level.dat, data/, …) minus its chunk data, plus our region files.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use rayon::prelude::*;

use crate::chunk::{Chunk, PaletteStyle};
use crate::nbt_write::encode_chunk;
use crate::region::write_region;
use crate::registry::BlockRegistry;
use crate::world::region_dir;

/// Per-dimension dirs holding chunk-derived data that must not leak from the template.
const CHUNK_DIRS: [&str; 3] = ["region", "entities", "poi"];

pub struct WorldWriter {
    region_dir: PathBuf,
    registry: Arc<BlockRegistry>,
    style: PaletteStyle,
}

/// Template world files as (path relative to the world root with `/` separators, bytes).
pub type TemplateFiles = Vec<(String, Vec<u8>)>;

/// Everything of a template world except chunk data and the session lock.
pub fn read_template(dir: &Path) -> Result<TemplateFiles> {
    if !dir.join("level.dat").exists() {
        bail!("template {} has no level.dat (py -3 tools/make_world.py template-void)", dir.display());
    }
    let mut out = Vec::new();
    collect(dir, "", &mut out)?;
    Ok(out)
}

fn collect(dir: &Path, prefix: &str, out: &mut TemplateFiles) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "session.lock" || CHUNK_DIRS.contains(&name.as_str()) {
            continue;
        }
        let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        if entry.file_type()?.is_dir() {
            collect(&entry.path(), &rel, out)?;
        } else {
            out.push((rel, fs::read(entry.path())?));
        }
    }
    Ok(())
}

impl WorldWriter {
    /// Creates `out` from template files; folder layout follows the template (it has `dimensions/` from
    /// 26.1 on), palettes are written in `style` (the target version's). Refuses to overwrite a world.
    pub fn create(
        out: &Path,
        template: &[(String, Vec<u8>)],
        dimension: &str,
        registry: Arc<BlockRegistry>,
        style: PaletteStyle,
    ) -> Result<Self> {
        if out.join("level.dat").exists() {
            bail!("{} already contains a world", out.display());
        }
        for (rel, bytes) in template {
            let path = out.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, bytes).with_context(|| path.display().to_string())?;
        }
        Self::for_dimension(out, template, dimension, registry, style)
    }

    /// Adds `dimension` to a world `create` made from the same template (one world, several dimensions).
    /// Refuses a dimension that already has chunks.
    pub fn extend(
        out: &Path,
        template: &[(String, Vec<u8>)],
        dimension: &str,
        registry: Arc<BlockRegistry>,
        style: PaletteStyle,
    ) -> Result<Self> {
        if !out.join("level.dat").exists() {
            bail!("{} has no world to add {dimension} to", out.display());
        }
        let writer = Self::for_dimension(out, template, dimension, registry, style)?;
        if fs::read_dir(&writer.region_dir)?.next().is_some() {
            bail!("{} already has chunks for {dimension}", out.display());
        }
        Ok(writer)
    }

    fn for_dimension(
        out: &Path,
        template: &[(String, Vec<u8>)],
        dimension: &str,
        registry: Arc<BlockRegistry>,
        style: PaletteStyle,
    ) -> Result<Self> {
        let modern = template.iter().any(|(rel, _)| rel.starts_with("dimensions/"));
        let region_dir = region_dir(out, dimension, modern);
        fs::create_dir_all(&region_dir)?;
        Ok(Self { region_dir, registry, style })
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
                    let nbt = encode_chunk(c, &self.registry, self.style)
                        .with_context(|| format!("chunk {},{}", c.x, c.z))?;
                    Ok(((c.x.rem_euclid(32) as u8, c.z.rem_euclid(32) as u8), nbt))
                })
                .collect::<Result<Vec<_>>>()?;
            write_region(&self.region_dir.join(format!("r.{rx}.{rz}.mca")), &encoded)
        })?;
        Ok(count)
    }
}
