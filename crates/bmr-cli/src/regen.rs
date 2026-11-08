//! `bmr pull --regen-seed`: generate a same-seed vanilla world for the chunks a site rendered, with a real
//! server run by Java. Opt-in; the default `pull` path needs neither Java nor a server. See docs/seed.md.

mod chunks;
mod java;
mod mojang;
mod server;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
pub use chunks::rendered_chunks;
use chunks::{Chunk, ChunkRect, batches, rectangles};
use java::Java;
use serde::Deserialize;
use server::{Launch, Server, absolute, free_port, properties};

use crate::ui::progress;

const EULA_URL: &str = "https://aka.ms/MinecraftEULA";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(600);
/// A server busy generating can take minutes to answer anything.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(300);
const BATCH_TIMEOUT: Duration = Duration::from_secs(1800);
const STOP_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(clap::Args)]
#[group(id = "RegenArgs")]
pub struct Args {
    /// Generate the same-seed vanilla world for the rendered chunks with a real server (needs Java), then
    /// fill from it: the seed as number or text, or a `*.seed.json` report from --crack-seed
    #[arg(long, allow_hyphen_values = true, conflicts_with = "regen", value_name = "SEED")]
    regen_seed: Option<String>,
    /// Accept the Minecraft EULA <https://aka.ms/MinecraftEULA>, required to run the server for --regen-seed
    #[arg(long)]
    accept_eula: bool,
    /// Java binary for --regen-seed [default: `java` on PATH]
    #[arg(long, value_name = "PATH")]
    java: Option<PathBuf>,
    /// Use this vanilla server jar instead of downloading the pack's version
    #[arg(long, value_name = "JAR")]
    server_jar: Option<PathBuf>,
    /// Java heap for the server
    #[arg(long, default_value = "3G", value_name = "SIZE")]
    server_heap: String,
    /// The site's world type
    #[arg(long, default_value = "minecraft:normal", value_name = "TYPE")]
    level_type: String,
}

/// What the regen world is generated for.
pub struct Site<'a> {
    pub data_dir: &'a Path,
    pub mirror: &'a Path,
    pub mc: &'a str,
    /// (dimension, rendered chunks) per map.
    pub maps: Vec<(&'a str, BTreeSet<Chunk>)>,
}

impl Args {
    /// `None` without `--regen-seed`. Runs first so a missing Java or EULA fails before a long mirror.
    pub fn preflight(&self) -> Result<Option<Java>> {
        if self.regen_seed.is_none() {
            return Ok(None);
        }
        if !self.accept_eula {
            bail!("--regen-seed runs the Minecraft server: read {EULA_URL} and pass --accept-eula if you agree");
        }
        java::detect(self.java.as_deref()).map(Some)
    }

    /// Server dir of a finished generation for this seed and chunk set is reused; anything else is rebuilt.
    pub fn generate(&self, java: &Java, site: &Site<'_>) -> Result<PathBuf> {
        let seed = resolve_seed(self.regen_seed.as_deref().context("no seed")?)?;
        let cache = site.data_dir.join("minecraft");
        let dir = site.mirror.join("regen");
        let marker = dir.join("bmr-regen.sha1");
        let digest = digest(site, &seed, &self.level_type);
        if std::fs::read_to_string(&marker).is_ok_and(|m| m == digest) {
            progress!("      reusing the regen world in {}", dir.display());
            return Ok(dir.join("world"));
        }
        let jar = self.server_jar_for(java, &cache, site.mc)?;
        if dir.exists() {
            std::fs::remove_dir_all(&dir).with_context(|| dir.display().to_string())?;
        }
        std::fs::create_dir_all(&dir).with_context(|| dir.display().to_string())?;
        std::fs::write(dir.join("eula.txt"), "eula=true\n")?;
        std::fs::write(dir.join("server.properties"), properties(&seed, &self.level_type, free_port()?))?;

        let started = Instant::now();
        let java_path = absolute(&java.path)?;
        let libraries = absolute(&cache.join("libraries"))?;
        let launch = Launch { java: &java_path, jar: &jar, heap: &self.server_heap, libraries: &libraries, dir: &dir };
        progress!("      starting the Minecraft {} server (seed {seed}, Java {})", site.mc, java.major);
        let mut server = Server::start(&launch)?;
        server.wait_for(&["Done ("], STARTUP_TIMEOUT)?;
        for (dimension, chunks) in &site.maps {
            pregenerate(&mut server, dimension, chunks)?;
        }
        server
            .query("save-all flush", &["Saved the game"], ANSWER_TIMEOUT)
            .context("the server did not finish saving the world (short on memory or disk?); a re-run starts over")?;
        server.stop(STOP_TIMEOUT)?;
        if !server.errors.is_empty() {
            bail!("the server refused commands:\n  {}", server.errors.join("\n  "));
        }
        std::fs::write(&marker, digest)?;
        progress!("      regen world ready in {:.1?}: {}", started.elapsed(), dir.display());
        Ok(dir.join("world"))
    }

    /// The jar to run, after checking Java against what Mojang lists for `mc`. With a local `--server-jar` the
    /// metadata is optional, so an offline run still works.
    fn server_jar_for(&self, java: &Java, cache: &Path, mc: &str) -> Result<PathBuf> {
        let info = mojang::version_info(cache, mc);
        if let Some(jar) = &self.server_jar {
            match info {
                Ok(i) => java::ensure_at_least(java, i.java_major, mc)?,
                Err(e) => println!("      note: cannot check the Java version against Minecraft {mc}: {e:#}"),
            }
            return absolute(jar);
        }
        let info = info?;
        java::ensure_at_least(java, info.java_major, mc)?;
        absolute(&mojang::server_jar(cache, mc, &info)?)
    }
}

/// Force-load each batch until its chunks are generated, then release it (see [`chunks::batches`]).
fn pregenerate(server: &mut Server, dimension: &str, chunks: &BTreeSet<Chunk>) -> Result<()> {
    let rects = rectangles(chunks);
    let groups = batches(&rects);
    for (i, batch) in groups.iter().enumerate() {
        progress!("      generating {dimension}: batch {}/{} ({} chunks total)", i + 1, groups.len(), chunks.len());
        for r in *batch {
            server.send(&r.forceload(dimension, "add"))?;
        }
        wait_until_loaded(server, dimension, batch)?;
        for r in *batch {
            server.send(&r.forceload(dimension, "remove"))?;
        }
    }
    Ok(())
}

fn wait_until_loaded(server: &mut Server, dimension: &str, batch: &[ChunkRect]) -> Result<()> {
    let deadline = Instant::now() + BATCH_TIMEOUT;
    for (x, z) in batch.iter().flat_map(ChunkRect::corners) {
        let probe = format!("execute in {dimension} if loaded {x} 0 {z}");
        while server.query(&probe, &["Test passed", "Test failed"], ANSWER_TIMEOUT)? != "Test passed" {
            if Instant::now() > deadline {
                bail!("chunk at {x},{z} never loaded");
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    Ok(())
}

/// Identifies what a finished regen world was generated for.
fn digest(site: &Site<'_>, seed: &str, level_type: &str) -> String {
    let mut text = format!("{}\0{seed}\0{level_type}\0", site.mc);
    for (dimension, chunks) in &site.maps {
        text.push_str(dimension);
        for (x, z) in chunks {
            text.push_str(&format!("\0{x},{z}"));
        }
    }
    mojang::sha1_hex(text.as_bytes())
}

#[derive(Deserialize)]
struct SeedFile {
    report: SeedReport,
}

#[derive(Deserialize)]
struct SeedReport {
    world_seed: Option<i64>,
}

/// The seed for server.properties: a `*.seed.json` report's world seed, else the argument itself.
fn resolve_seed(arg: &str) -> Result<String> {
    let path = Path::new(arg);
    if !(arg.ends_with(".json") && path.is_file()) {
        return Ok(arg.to_owned());
    }
    let bytes = std::fs::read(path).with_context(|| arg.to_owned())?;
    let file: SeedFile = serde_json::from_slice(&bytes).with_context(|| format!("{arg}: not a seed report"))?;
    match file.report.world_seed {
        Some(s) => Ok(s.to_string()),
        None => bail!("{arg}: the crack found no single world seed; pass the seed itself"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_argument_is_taken_literally_unless_it_is_a_report() {
        assert_eq!(resolve_seed("-123").unwrap(), "-123");
        assert_eq!(resolve_seed("bluemap_reverse").unwrap(), "bluemap_reverse");
        assert_eq!(resolve_seed("missing.seed.json").unwrap(), "missing.seed.json");
    }

    #[test]
    fn reads_the_world_seed_from_a_report() {
        let dir = std::env::temp_dir().join(format!("bmr-regen-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let found = dir.join("found.seed.json");
        std::fs::write(&found, r#"{"mc":"26.3","structures":[],"report":{"world_seed":-5}}"#).unwrap();
        let none = dir.join("none.seed.json");
        std::fs::write(&none, r#"{"report":{"world_seed":null}}"#).unwrap();
        assert_eq!(resolve_seed(found.to_str().unwrap()).unwrap(), "-5");
        assert!(resolve_seed(none.to_str().unwrap()).unwrap_err().to_string().contains("no single world seed"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn digest_changes_with_seed_and_chunks() {
        let chunks: BTreeSet<Chunk> = [(0, 0)].into_iter().collect();
        let site = |maps| Site { data_dir: Path::new("d"), mirror: Path::new("m"), mc: "26.3", maps };
        let a = digest(&site(vec![("minecraft:overworld", chunks.clone())]), "1", "minecraft:normal");
        assert_ne!(a, digest(&site(vec![("minecraft:overworld", chunks.clone())]), "2", "minecraft:normal"));
        assert_ne!(a, digest(&site(vec![("minecraft:overworld", BTreeSet::new())]), "1", "minecraft:normal"));
    }
}
