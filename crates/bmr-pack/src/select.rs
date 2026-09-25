//! Choose a pack for a site. Sites publish their BlueMap version but not their Minecraft version; the
//! texture list in `textures.json` is a reliable fingerprint of the latter (every release adds/renames
//! textures), so packs are ranked by how much of it they know, BlueMap version breaking ties.

use std::path::PathBuf;

use crate::{Header, Pack};

pub struct Ranked {
    pub path: PathBuf,
    pub header: Header,
    /// Share of the site's textures the pack knows.
    pub coverage: f64,
    /// Share of the pack's textures the site has (tells an older site from a newer pack).
    pub overlap: f64,
    pub bluemap_match: bool,
}

/// Best first. Unreadable files are skipped (and returned as warnings).
pub fn rank(paths: &[PathBuf], site_version: Option<&str>, site_textures: &[String]) -> (Vec<Ranked>, Vec<String>) {
    let site: std::collections::HashSet<&str> = site_textures.iter().map(String::as_str).collect();
    let mut ranked = Vec::new();
    let mut warnings = Vec::new();
    for path in paths {
        let header = match Pack::read_header(path) {
            Ok(h) => h,
            Err(e) => {
                warnings.push(format!("{e:#}"));
                continue;
            }
        };
        let pack: std::collections::HashSet<&str> = header.site_textures.iter().map(String::as_str).collect();
        let known = site.iter().filter(|t| pack.contains(*t)).count();
        let coverage = known as f64 / site.len().max(1) as f64;
        let overlap = known as f64 / pack.len().max(1) as f64;
        let bluemap_match = site_version.is_some_and(|v| v == header.meta.bluemap_version);
        ranked.push(Ranked { path: path.clone(), header, coverage, overlap, bluemap_match });
    }
    // coverage first (can we read the site at all), then how exactly the version fits, then BlueMap
    ranked.sort_by(|a, b| {
        (b.coverage, b.overlap, b.bluemap_match as u8)
            .partial_cmp(&(a.coverage, a.overlap, a.bluemap_match as u8))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    (ranked, warnings)
}

impl Ranked {
    pub fn describe(&self) -> String {
        format!(
            "{}: Minecraft {}, BlueMap {} — knows {:.1}% of the site's textures, {:.1}% of its own are on the site",
            self.path.file_name().map_or_else(|| self.path.display().to_string(), |n| n.to_string_lossy().into_owned()),
            self.header.meta.mc_version,
            self.header.meta.bluemap_version,
            100.0 * self.coverage,
            100.0 * self.overlap
        )
    }
}
