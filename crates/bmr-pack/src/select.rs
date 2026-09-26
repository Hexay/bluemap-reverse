//! Choose a pack for a site. Sites publish their BlueMap version but not their Minecraft version; the
//! texture list in `textures.json` is a reliable fingerprint of the latter (every release adds/renames
//! textures), so packs are ranked by how much of it they know, BlueMap version breaking ties.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::{Index, IndexEntry, Pack};

pub enum Source {
    Installed(PathBuf),
    /// Listed in a pack index, not downloaded yet.
    Indexed(IndexEntry),
}

pub struct Candidate {
    pub source: Source,
    pub mc_version: String,
    pub bluemap_version: String,
    pub textures: Vec<String>,
}

pub struct Ranked {
    pub candidate: Candidate,
    /// Share of the site's textures the pack knows.
    pub coverage: f64,
    /// Share of the pack's textures the site has (tells an older site from a newer pack).
    pub overlap: f64,
    pub bluemap_match: bool,
}

/// Installed packs by header; unreadable files are returned as warnings.
pub fn installed(paths: &[PathBuf]) -> (Vec<Candidate>, Vec<String>) {
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    for path in paths {
        match Pack::read_header(path) {
            Ok(h) => out.push(Candidate {
                source: Source::Installed(path.clone()),
                mc_version: h.meta.mc_version,
                bluemap_version: h.meta.bluemap_version,
                textures: h.site_textures,
            }),
            Err(e) => warnings.push(format!("{e:#}")),
        }
    }
    (out, warnings)
}

/// Readable index entries whose file is not installed under the same name.
pub fn indexed(index: &Index, installed: &[Candidate]) -> anyhow::Result<Vec<Candidate>> {
    let have: HashSet<String> = installed.iter().filter_map(|c| c.file_name()).collect();
    index
        .usable()
        .filter(|e| !have.contains(&e.file))
        .map(|e| {
            Ok(Candidate {
                mc_version: e.mc_version.clone(),
                bluemap_version: e.bluemap_version.clone(),
                textures: index.textures_of(e)?,
                source: Source::Indexed(e.clone()),
            })
        })
        .collect()
}

/// Best first; on equal scores installed packs come first.
pub fn rank(candidates: Vec<Candidate>, site_version: Option<&str>, site_textures: &[String]) -> Vec<Ranked> {
    let site: HashSet<&str> = site_textures.iter().map(String::as_str).collect();
    let mut ranked: Vec<Ranked> = candidates
        .into_iter()
        .map(|c| {
            let pack: HashSet<&str> = c.textures.iter().map(String::as_str).collect();
            let known = site.iter().filter(|t| pack.contains(*t)).count();
            Ranked {
                coverage: known as f64 / site.len().max(1) as f64,
                overlap: known as f64 / pack.len().max(1) as f64,
                bluemap_match: site_version.is_some_and(|v| v == c.bluemap_version),
                candidate: c,
            }
        })
        .collect();
    // coverage first (can we read the site at all), then how exactly the version fits, then BlueMap
    ranked.sort_by(|a, b| {
        (b.coverage, b.overlap, b.bluemap_match as u8)
            .partial_cmp(&(a.coverage, a.overlap, a.bluemap_match as u8))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked
}

impl Candidate {
    pub fn file_name(&self) -> Option<String> {
        match &self.source {
            Source::Installed(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()),
            Source::Indexed(e) => Some(e.file.clone()),
        }
    }
}

impl Ranked {
    pub fn describe(&self) -> String {
        let c = &self.candidate;
        format!(
            "{}{}: Minecraft {}, BlueMap {} — knows {:.1}% of the site's textures, {:.1}% of its own are on the site",
            c.file_name().unwrap_or_default(),
            if matches!(c.source, Source::Indexed(_)) { " (download)" } else { "" },
            c.mc_version,
            c.bluemap_version,
            100.0 * self.coverage,
            100.0 * self.overlap
        )
    }
}
