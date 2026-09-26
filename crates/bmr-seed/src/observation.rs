//! Observed structure starts → per-region constraints on the structure seed.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::placement::{region_of, region_offset};
use crate::structure_set::{Spread, StructureSet, vanilla};

/// A structure seen on the map. `chunks` lists the start chunks it could have (usually one; several when the
/// blocks leave it ambiguous, e.g. which template corner sits on the chunk). Reads `"chunk": [x, z]` too.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(from = "RawObservation")]
pub struct Observation {
    /// structure_set name, e.g. "desert_pyramids"
    pub set: String,
    pub chunks: Vec<[i32; 2]>,
}

#[derive(Deserialize)]
struct RawObservation {
    set: String,
    chunk: Option<[i32; 2]>,
    #[serde(default)]
    chunks: Vec<[i32; 2]>,
}

impl From<RawObservation> for Observation {
    fn from(r: RawObservation) -> Self {
        Self { set: r.set, chunks: r.chunk.into_iter().chain(r.chunks).collect() }
    }
}

/// One possible start chunk of an observation.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub chunk: (i32, i32),
    /// `placement::region_offset` of its region
    pub offset: i64,
    /// in-region offset
    pub target: (i32, i32),
}

#[derive(Clone, Debug)]
pub struct Constraint {
    pub set: StructureSet,
    /// possible start chunks inside the set's range; empty = the observation cannot be this set (misidentified,
    /// modded)
    pub options: Vec<Placement>,
    /// low bits of each axis readable from `seed mod 2^(17+lift)` (0 = not liftable); see solve.rs
    pub lift: u32,
}

impl Constraint {
    pub fn feasible(&self) -> bool {
        !self.options.is_empty()
    }

    /// Information about the structure seed, in bits (offset entropy, less what the alternatives cost).
    pub fn bits(&self) -> f64 {
        2.0 * axis_entropy(self.set.range(), self.set.spread) - (self.options.len().max(1) as f64).log2()
    }
}

/// Upper bound on what the constraints say about the 48-bit structure seed. The liftable low bits of every
/// structure fall in the same 17+K low seed bits, so together they are worth at most 17+K.
pub fn total_bits(constraints: &[Constraint]) -> f64 {
    let feasible = constraints.iter().filter(|c| c.feasible());
    let lifted: f64 = feasible.clone().map(|c| 2.0 * c.lift as f64).sum();
    let low_cap = 17.0 + feasible.clone().map(|c| c.lift).max().unwrap_or(0) as f64;
    lifted.min(low_cap) + feasible.map(|c| (c.bits() - 2.0 * c.lift as f64).max(0.0)).sum::<f64>()
}

fn axis_entropy(r: i32, spread: Spread) -> f64 {
    match spread {
        Spread::Linear => (r as f64).log2(),
        Spread::Triangular => {
            let mut counts = vec![0u32; r as usize];
            for a in 0..r {
                for b in 0..r {
                    counts[((a + b) / 2) as usize] += 1;
                }
            }
            let n = (r * r) as f64;
            counts.iter().filter(|&&c| c > 0).map(|&c| -(c as f64 / n) * (c as f64 / n).log2()).sum()
        }
    }
}

fn lift_bits(set: &StructureSet) -> u32 {
    let r = set.range();
    let power_of_two = r & (r - 1) == 0;
    if set.spread == Spread::Linear && !power_of_two { r.trailing_zeros() } else { 0 }
}

fn placement(set: &StructureSet, chunk: [i32; 2]) -> Option<Placement> {
    let chunk = (chunk[0], chunk[1]);
    let (rx, rz) = region_of(set, chunk);
    let target = (chunk.0 - rx * set.spacing, chunk.1 - rz * set.spacing);
    let r = set.range();
    ((0..r).contains(&target.0) && (0..r).contains(&target.1))
        .then(|| Placement { chunk, offset: region_offset(set, rx, rz), target })
}

pub fn constraints(mc: &str, observations: &[Observation]) -> Result<Vec<Constraint>> {
    let sets = vanilla(mc).with_context(|| format!("no structure placement data for Minecraft {mc} (1.18.2+ only)"))?;
    if observations.is_empty() {
        bail!("no observations");
    }
    observations
        .iter()
        .map(|o| {
            let set = *sets
                .iter()
                .find(|s| s.name == o.set)
                .with_context(|| format!("unknown structure set {:?} for {mc}", o.set))?;
            let options = o.chunks.iter().filter_map(|&c| placement(&set, c)).collect();
            Ok(Constraint { set, options, lift: lift_bits(&set) })
        })
        .collect()
}
