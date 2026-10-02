use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::paths::DataArgs;
use crate::ui::{fetch_progress, progress};

#[derive(clap::Args)]
pub struct Args {
    /// Webapp root, e.g. <http://127.0.0.1:8100/>
    url: String,
    /// Mirror dir [default: DATA/cache/HOST_PORT, see --data-dir]
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// Only these map ids (comma-separated)
    #[arg(long, value_delimiter = ',')]
    maps: Vec<String>,
    #[command(flatten)]
    download: DownloadArgs,
}

/// Download settings shared by `fetch` and `pull`.
#[derive(clap::Args)]
pub struct DownloadArgs {
    #[command(flatten)]
    pub data: DataArgs,
    /// Parallel downloads
    #[arg(long, default_value_t = 4)]
    pub concurrency: usize,
    /// Pause before each request per download worker (be gentle with other people's servers)
    #[arg(long, default_value_t = 25)]
    pub delay_ms: u64,
}

impl DownloadArgs {
    /// Mirror options for `url` into `out`; empty `maps` = every map.
    pub fn options(&self, url: &str, out: PathBuf, maps: Vec<String>) -> bmr_fetch::Options {
        bmr_fetch::Options {
            base_url: url.to_owned(),
            out,
            maps,
            concurrency: self.concurrency,
            delay: Duration::from_millis(self.delay_ms),
            progress: fetch_progress(),
        }
    }
}

pub fn run(a: Args) -> Result<()> {
    let out = match a.out {
        Some(out) => out,
        None => a.download.data.mirror_dir(&a.url)?,
    };
    let opts = a.download.options(&a.url, out.clone(), a.maps);
    let t = Instant::now();
    for m in bmr_fetch::mirror(&opts)? {
        for (lod, present, empty) in m.layers {
            progress!("{:<16} lod {lod}: {present:>6} tiles ({empty} empty probes)", m.id);
        }
    }
    println!("mirrored to {} in {:.1?}", out.display(), t.elapsed());
    Ok(())
}
