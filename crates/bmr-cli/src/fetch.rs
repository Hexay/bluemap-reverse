use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;

#[derive(clap::Args)]
pub struct Args {
    /// Webapp root, e.g. http://127.0.0.1:8100/
    url: String,
    /// Output dir [default: work/cache/<host_port>]
    #[arg(long)]
    out: Option<PathBuf>,
    /// Only these map ids (comma-separated)
    #[arg(long, value_delimiter = ',')]
    maps: Vec<String>,
    #[arg(long, default_value_t = 4)]
    concurrency: usize,
    /// Sleep before every request, per worker
    #[arg(long, default_value_t = 0)]
    delay_ms: u64,
}

pub fn run(a: Args) -> Result<()> {
    let out = a.out.unwrap_or_else(|| PathBuf::from("work/cache").join(site_slug(&a.url)));
    let opts = bmr_fetch::Options {
        base_url: a.url,
        out: out.clone(),
        maps: a.maps,
        concurrency: a.concurrency,
        delay: Duration::from_millis(a.delay_ms),
    };
    let t = Instant::now();
    for m in bmr_fetch::mirror(&opts)? {
        for (lod, present, empty) in m.layers {
            println!("{:<16} lod {lod}: {present:>6} tiles ({empty} empty probes)", m.id);
        }
    }
    println!("mirrored to {} in {:.1?}", out.display(), t.elapsed());
    Ok(())
}

pub fn site_slug(url: &str) -> String {
    let host = url.split("://").nth(1).unwrap_or(url);
    host.trim_end_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect()
}
