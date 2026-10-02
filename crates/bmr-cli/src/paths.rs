//! Where user-facing commands keep downloads: the per-user data dir, with site mirrors in `cache/` and
//! downloaded packs in `packs/`. Resolved from `--data-dir`, else `$BMR_HOME`, else the platform's data dir.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const APP: &str = "bluemap-reverse";
pub const PACKS: &str = "packs";

#[derive(clap::Args)]
pub struct DataArgs {
    /// Data dir for downloads: site mirrors in `DIR/cache`, packs in `DIR/packs` [default: $BMR_HOME, else
    /// %LOCALAPPDATA%\bluemap-reverse, ~/Library/Application Support/bluemap-reverse or
    /// $XDG_DATA_HOME/bluemap-reverse]
    #[arg(long, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,
}

impl DataArgs {
    pub fn data_dir(&self) -> Result<PathBuf> {
        data_dir(self.data_dir.as_deref())
    }

    /// `<data>/cache/<site>` for the site at `url`.
    pub fn mirror_dir(&self, url: &str) -> Result<PathBuf> {
        Ok(self.data_dir()?.join("cache").join(site_slug(url)))
    }
}

pub fn data_dir(flag: Option<&Path>) -> Result<PathBuf> {
    if let Some(dir) = flag {
        return Ok(dir.to_path_buf());
    }
    if let Some(dir) = env_path("BMR_HOME") {
        return Ok(dir);
    }
    platform_data_dir().map(|d| d.join(APP)).context("no per-user data dir found: set BMR_HOME or pass --data-dir")
}

fn platform_data_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        env_path("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        env_path("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        // the XDG spec says to ignore relative values
        env_path("XDG_DATA_HOME")
            .filter(|p| p.is_absolute())
            .or_else(|| env_path("HOME").map(|h| h.join(".local/share")))
    }
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// File-name-safe `host[_port]` of a site URL.
pub fn site_slug(url: &str) -> String {
    let host = url.split("://").nth(1).unwrap_or(url);
    host.trim_end_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect()
}
