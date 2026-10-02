//! `bmr structures`: find structures in a (reconstructed) world and write them as `bmr seed` observations.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
use bmr_seed::Observation;
use bmr_seed::detect::{detect, evaluate};
use serde::Deserialize;

use crate::WorldArgs;

#[derive(clap::Args)]
pub struct Args {
    /// World root (a reconstruction or any world)
    world: PathBuf,
    /// Write observations for `bmr seed` here
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// Minecraft version recorded in the observations file
    #[arg(long, default_value = "26.3")]
    mc: String,
    /// Known starts (tools/structure_truth.py output): print detection accuracy per structure type
    #[arg(long)]
    truth: Option<PathBuf>,
    /// Write every detection (bbox, block count, candidate chunks) as JSON here
    #[arg(long)]
    detections: Option<PathBuf>,
    #[command(flatten)]
    world_args: WorldArgs,
}

#[derive(Deserialize)]
struct Truth {
    structures: Vec<Observation>,
}

pub fn run(a: Args) -> Result<()> {
    let registry = a.world_args.registry()?;
    let world = a.world_args.open(&a.world, &registry)?;
    let scan = detect(&world)?;
    let usable = scan.detections.iter().filter(|d| !d.chunks.is_empty()).count();
    println!("{} detections ({usable} with candidate chunks) in {} chunks", scan.detections.len(), scan.chunks.len());

    if let Some(path) = &a.truth {
        let truth: Truth = serde_json::from_slice(&std::fs::read(path)?)?;
        let present: HashSet<[i32; 2]> = scan.chunks.iter().copied().collect();
        let scores = evaluate(&scan.detections, &truth.structures, |c| present.contains(&c));
        println!(
            "{:<20} {:>5} {:>5} {:>7} {:>9} {:>6} {:>7}",
            "set", "truth", "found", "correct", "misplaced", "false", "options"
        );
        for (set, s) in &scores {
            println!(
                "{set:<20} {:>5} {:>5} {:>7} {:>9} {:>6} {:>7.1}",
                s.truth, s.detections, s.correct, s.misplaced, s.false_positives, s.options
            );
        }
    }
    if let Some(path) = &a.detections {
        std::fs::write(path, serde_json::to_string_pretty(&scan.detections)?)?;
    }
    if let Some(out) = &a.out {
        let structures: Vec<Observation> =
            scan.detections.iter().filter(|d| !d.chunks.is_empty()).map(|d| d.observation()).collect();
        let doc = serde_json::json!({ "mc": a.mc, "structures": structures });
        std::fs::write(out, serde_json::to_string_pretty(&doc)?)?;
        println!("wrote {} observations to {}", structures.len(), out.display());
    }
    Ok(())
}
