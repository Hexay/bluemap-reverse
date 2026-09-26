//! Where packs come from: installed files (`./packs`, `packs/` next to bmr) and the pack index, whose
//! packs are downloaded on demand into `./packs` (see bmr-pack/src/index.rs).

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use bmr_pack::{Candidate, Index, IndexEntry, Source};

pub const DEFAULT_INDEX: &str = "https://github.com/Hexay/bluemap-reverse/releases/download/packs/index.json";
const DOWNLOAD_DIR: &str = "packs";

#[derive(clap::Args)]
pub struct IndexArgs {
    /// Pack index URL; its packs are downloaded from next to it
    #[arg(long, default_value = DEFAULT_INDEX)]
    pub pack_index: String,
    /// Installed packs only
    #[arg(long)]
    pub offline: bool,
}

/// Installed pack files: `packs/` in the working directory and next to the executable.
pub fn find_packs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from(DOWNLOAD_DIR)];
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        dirs.push(exe_dir.join(DOWNLOAD_DIR));
    }
    let mut out: Vec<PathBuf> = dirs
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "pack"))
        .collect();
    out.sort();
    out.dedup();
    out
}

pub fn fetch_index(url: &str) -> Result<Index> {
    let (base, name) = url.rsplit_once('/').with_context(|| format!("bad index URL {url}"))?;
    let json = bmr_fetch::Http::new(base, Duration::ZERO)?.get(name)?.with_context(|| format!("no pack index at {url}"))?;
    Index::parse(&json)
}

/// Downloads, verifies against the index and installs into `./packs`.
pub fn download(index_url: &str, entry: &IndexEntry) -> Result<PathBuf> {
    let base = index_url.rsplit_once('/').map_or(index_url, |(b, _)| b);
    let bytes = bmr_fetch::Http::new(base, Duration::ZERO)?
        .get(&entry.file)?
        .with_context(|| format!("{} is listed in the index but not downloadable", entry.file))?;
    entry.verify(&bytes)?;
    std::fs::create_dir_all(DOWNLOAD_DIR)?;
    let path = Path::new(DOWNLOAD_DIR).join(&entry.file);
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Installed packs plus, unless offline, the index's others. Problems become printable notes.
pub fn candidates(args: &IndexArgs) -> (Vec<Candidate>, Vec<String>) {
    let (mut all, mut notes) = bmr_pack::installed(&find_packs());
    notes.iter_mut().for_each(|n| *n = format!("skipped: {n}"));
    if !args.offline {
        match fetch_index(&args.pack_index).and_then(|index| bmr_pack::indexed(&index, &all)) {
            Ok(more) => all.extend(more),
            Err(e) => notes.push(format!("pack index unavailable ({e:#}); using installed packs")),
        }
    }
    (all, notes)
}

/// The pack to use: explicit, else the best fit for the site's texture fingerprint among installed and
/// indexed packs (downloading it if needed). Also returns the ranking as printable lines, best first.
pub fn select_pack(
    explicit: Option<&Path>,
    args: &IndexArgs,
    site_version: Option<&str>,
    site_textures: &[String],
) -> Result<(PathBuf, Vec<String>)> {
    if let Some(p) = explicit {
        return Ok((p.to_path_buf(), Vec::new()));
    }
    let (candidates, notes) = candidates(args);
    if candidates.is_empty() {
        bail!("no pack installed or indexed ({}); pass --pack <file>", notes.join("; "));
    }
    let ranked = bmr_pack::rank(candidates, site_version, site_textures);
    let mut lines: Vec<String> = ranked.iter().map(|r| r.describe()).collect();
    lines.extend(notes);
    let best = ranked.into_iter().next().expect("candidates is non-empty").candidate;
    let path = match best.source {
        Source::Installed(p) => p,
        Source::Indexed(e) => {
            let p = download(&args.pack_index, &e)?;
            lines.push(format!("downloaded {} ({:.1} MB)", p.display(), e.size as f64 / 1e6));
            p
        }
    };
    Ok((path, lines))
}
