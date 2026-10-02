//! Where packs come from: installed files (`packs/` next to bmr, `./packs`, `<data>/packs`) and the pack
//! index, whose packs are downloaded on demand into `<data>/packs` (see bmr-pack/src/index.rs, paths.rs).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use bmr_pack::{Candidate, Index, IndexEntry, Source};

use crate::paths::PACKS;
use crate::ui::fetch_progress;

pub const DEFAULT_INDEX: &str = "https://github.com/Hexay/bluemap-reverse/releases/download/packs/index.json";

#[derive(clap::Args)]
pub struct IndexArgs {
    /// Pack index URL; its packs are downloaded from next to it
    #[arg(long, default_value = DEFAULT_INDEX)]
    pub pack_index: String,
    /// Installed packs only
    #[arg(long)]
    pub offline: bool,
}

/// Installed pack files: `packs/` next to the executable (release archives bundle them), in the working
/// directory (a checkout) and in the data dir (downloads), in that order.
pub fn find_packs(data_dir: &Path) -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
    let dirs = exe_dir.map(|d| d.join(PACKS)).into_iter().chain([PathBuf::from(PACKS), data_dir.join(PACKS)]);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut packs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "pack"))
            .collect();
        packs.sort();
        // order matters: on equal fit `rank` keeps the earlier pack
        out.extend(packs.into_iter().filter(|p| seen.insert(std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()))));
    }
    out
}

pub fn fetch_index(url: &str) -> Result<Index> {
    let (base, name) = url.rsplit_once('/').with_context(|| format!("bad index URL {url}"))?;
    let http = bmr_fetch::Http::new(base, Duration::ZERO)?.with_progress(fetch_progress());
    let json = http.get(name)?.with_context(|| format!("no pack index at {url}"))?;
    Index::parse(&json)
}

/// Downloads, verifies against the index and installs into `<data_dir>/packs`.
pub fn download(index_url: &str, entry: &IndexEntry, data_dir: &Path) -> Result<PathBuf> {
    let base = index_url.rsplit_once('/').map_or(index_url, |(b, _)| b);
    let bytes = bmr_fetch::Http::new(base, Duration::ZERO)?
        .with_progress(fetch_progress())
        .get(&entry.file)?
        .with_context(|| format!("{} is listed in the index but not downloadable", entry.file))?;
    entry.verify(&bytes)?;
    let dir = data_dir.join(PACKS);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(&entry.file);
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Installed packs plus, unless offline, the index's others. Problems become printable notes.
pub fn candidates(args: &IndexArgs, data_dir: &Path) -> (Vec<Candidate>, Vec<String>) {
    let (mut all, mut notes) = bmr_pack::installed(&find_packs(data_dir));
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
    data_dir: &Path,
    site_version: Option<&str>,
    site_textures: &[String],
) -> Result<(PathBuf, Vec<String>)> {
    if let Some(p) = explicit {
        return Ok((p.to_path_buf(), Vec::new()));
    }
    let (candidates, notes) = candidates(args, data_dir);
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
            let p = download(&args.pack_index, &e, data_dir)?;
            lines.push(format!("downloaded {} ({:.1} MB)", p.display(), e.size as f64 / 1e6));
            p
        }
    };
    Ok((path, lines))
}
