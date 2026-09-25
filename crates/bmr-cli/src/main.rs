use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bmr", about = "Reconstruct a Minecraft world from a BlueMap web map")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Mirror a BlueMap site (only maps you own or may reverse).
    Fetch {
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
    },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Fetch { url, out, maps, concurrency, delay_ms } => {
            let out = out.unwrap_or_else(|| PathBuf::from("work/cache").join(site_slug(&url)));
            let opts = bmr_fetch::Options {
                base_url: url,
                out: out.clone(),
                maps,
                concurrency,
                delay: Duration::from_millis(delay_ms),
            };
            let t = Instant::now();
            for m in bmr_fetch::mirror(&opts)? {
                for (lod, present, empty) in m.layers {
                    println!("{:<16} lod {lod}: {present:>6} tiles ({empty} empty probes)", m.id);
                }
            }
            println!("mirrored to {} in {:.1?}", out.display(), t.elapsed());
        }
    }
    Ok(())
}

fn site_slug(url: &str) -> String {
    let host = url.split("://").nth(1).unwrap_or(url);
    host.trim_end_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect()
}
