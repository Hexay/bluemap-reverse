//! Zip a world folder into a single download that extracts to `<folder name>/…` (drop into `saves/`).

use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::Path;

use anyhow::{Context, Result};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

/// Returns the number of files written.
pub fn zip_world(world_dir: &Path, out: &Path) -> Result<usize> {
    let root = world_dir.file_name().and_then(|n| n.to_str()).context("world dir has no name")?.to_owned();
    let mut zip = ZipWriter::new(BufWriter::new(File::create(out).with_context(|| out.display().to_string())?));
    let n = add_dir(&mut zip, world_dir, &root)?;
    zip.finish()?;
    Ok(n)
}

fn add_dir(zip: &mut ZipWriter<BufWriter<File>>, dir: &Path, prefix: &str) -> Result<usize> {
    let mut n = 0;
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = format!("{prefix}/{}", e.file_name().to_string_lossy());
        if e.file_type()?.is_dir() {
            zip.add_directory(format!("{name}/"), SimpleFileOptions::default())?;
            n += add_dir(zip, &e.path(), &name)?;
            continue;
        }
        // region files are zlib-compressed per chunk already
        let method = if name.ends_with(".mca") { CompressionMethod::Stored } else { CompressionMethod::Deflated };
        zip.start_file(name, SimpleFileOptions::default().compression_method(method).large_file(true))?;
        io::copy(&mut File::open(e.path())?, zip)?;
        n += 1;
    }
    Ok(n)
}
