//! `bmr seed`: crack the world seed from observed structure starts (docs/seed.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use bmr_seed::detect::detect;
use bmr_seed::{Observation, Report, crack, default_max_misses};
use bmr_world::World;
use serde::Deserialize;

#[derive(clap::Args)]
pub struct Args {
    /// JSON {"mc": "26.3", "structures": [{"set": "desert_pyramids", "chunk": [x, z]}, ...]}
    observations: PathBuf,
    /// Minecraft version (overrides the file's "mc")
    #[arg(long)]
    mc: Option<String>,
    /// Observations allowed to be wrong (player builds, misidentified structures); default: one per six
    #[arg(long)]
    max_misses: Option<usize>,
    /// Print the full report as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Deserialize)]
struct Input {
    mc: Option<String>,
    structures: Vec<Observation>,
}

/// Exit status when the crack finishes without a single world seed.
const NO_SEED: u8 = 3;

pub fn run(a: Args) -> Result<ExitCode> {
    let bytes = std::fs::read(&a.observations).with_context(|| format!("reading {}", a.observations.display()))?;
    let input: Input = serde_json::from_slice(&bytes)?;
    let mc = a.mc.or(input.mc).context("Minecraft version unknown: pass --mc")?;
    let max_misses = a.max_misses.unwrap_or_else(|| default_max_misses(input.structures.len()));
    let report = crack(&mc, &input.structures, max_misses)?;
    let status = if report.world_seed.is_some() { ExitCode::SUCCESS } else { ExitCode::from(NO_SEED) };
    if a.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(status)
}

/// `bmr pull --crack-seed`: detect structures in a reconstructed overworld, crack its seed, print the
/// result and write the full report as JSON to `report_path`. Returns the world seed if one was found.
pub fn crack_world(world: &World, mc: &str, report_path: &Path) -> Result<Option<i64>> {
    let scan = detect(world)?;
    let structures = crate::structures::observations(&scan);
    if structures.is_empty() {
        println!("      no structures found to crack the seed from");
        return Ok(None);
    }
    let report = crack(mc, &structures, default_max_misses(structures.len()))?;
    let doc = serde_json::json!({ "mc": mc, "structures": structures, "report": report });
    std::fs::write(report_path, serde_json::to_string_pretty(&doc)?)
        .with_context(|| report_path.display().to_string())?;
    print_report(&report);
    Ok(report.world_seed)
}

fn print_report(report: &Report) {
    let max_misses = report.max_misses;
    println!(
        "{} observations, <={:.0} bits, <={} misses, {} low-bit survivors",
        report.observations, report.bits, report.max_misses, report.low_survivors
    );
    for c in report.candidates.iter().take(10) {
        println!("structure seed {:>15}  matched {}/{}", c.structure_seed, c.matched, report.observations);
        for w in &c.world_seeds {
            let origin = w.origin.map(|o| format!(" ({o:?})")).unwrap_or_default();
            println!("  world seed {:>21}  biome-viable {}/{}{origin}", w.seed, w.viable, c.biome_checks);
        }
    }
    if !report.unique && report.candidates.len() > 1 {
        println!("{} structure seeds tie: add structures of other types", report.candidates.len());
    }
    match (report.world_seed, report.candidates.is_empty()) {
        (Some(seed), _) => println!("seed: {seed}"),
        (None, true) => {
            println!(
                "no structure seed fits: modded salts, pre-1.18 chunks, or more than {max_misses} wrong observations"
            )
        }
        (None, false) => println!("no single world seed: add structures in more biomes"),
    }
}
