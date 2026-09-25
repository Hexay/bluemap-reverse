use std::collections::HashMap;
use std::fmt;

use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct Accuracy {
    pub total: u64,
    /// Same block state (name + properties).
    pub exact: u64,
    /// Same block name.
    pub name: u64,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct Overlap {
    pub original: u64,
    pub reconstructed: u64,
    pub both: u64,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct Hits {
    pub total: u64,
    pub hits: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Confusion {
    pub original: String,
    pub reconstructed: String,
    pub count: u64,
}

/// See docs/plan.md "Scoring". `visible` approximates BlueMap visibility as "non-air with an air
/// neighbour (void below min y counts as air)" until bmr-model knows real culling.
#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub chunks: u64,
    pub partial_chunks_skipped: u64,
    pub columns: u64,
    /// Every voxel; dominated by air, so it flatters an empty reconstruction.
    pub all: Accuracy,
    /// Voxels non-air in either world — the headline number.
    pub occupied: Accuracy,
    pub visible: Accuracy,
    /// Non-air voxels.
    pub solid: Overlap,
    /// Topmost non-air block per column: same y and same state.
    pub surface: Hits,
    /// 4×4×4 biome cells with the same biome.
    pub biomes: Hits,
    pub confusions: Vec<Confusion>,
    #[serde(skip)]
    pub(crate) confusion_counts: HashMap<(String, String), u64>,
}

impl Report {
    pub(crate) fn merge(&mut self, o: Report) {
        self.chunks += o.chunks;
        self.partial_chunks_skipped += o.partial_chunks_skipped;
        self.columns += o.columns;
        for (a, b) in [(&mut self.all, o.all), (&mut self.occupied, o.occupied), (&mut self.visible, o.visible)] {
            a.total += b.total;
            a.exact += b.exact;
            a.name += b.name;
        }
        self.solid.original += o.solid.original;
        self.solid.reconstructed += o.solid.reconstructed;
        self.solid.both += o.solid.both;
        for (a, b) in [(&mut self.surface, o.surface), (&mut self.biomes, o.biomes)] {
            a.total += b.total;
            a.hits += b.hits;
        }
        for (k, v) in o.confusion_counts {
            *self.confusion_counts.entry(k).or_default() += v;
        }
    }

    pub(crate) fn finish(&mut self, top: usize) {
        let mut v: Vec<_> = self.confusion_counts.drain().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        self.confusions = v
            .into_iter()
            .take(top)
            .map(|((original, reconstructed), count)| Confusion { original, reconstructed, count })
            .collect();
    }

    pub fn solid_iou(&self) -> f64 {
        ratio(self.solid.both, self.solid.original + self.solid.reconstructed - self.solid.both)
    }
}

fn ratio(a: u64, b: u64) -> f64 {
    if b == 0 { 1.0 } else { a as f64 / b as f64 }
}

fn pct(a: u64, b: u64) -> String {
    format!("{:6.2}%", 100.0 * ratio(a, b))
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "chunks {} ({} partial skipped), columns {}", self.chunks, self.partial_chunks_skipped, self.columns)?;
        for (label, a) in [("occupied  ", self.occupied), ("visible   ", self.visible), ("all voxels", self.all)] {
            writeln!(f, "{label}  state {}  name {}  (n={})", pct(a.exact, a.total), pct(a.name, a.total), a.total)?;
        }
        writeln!(f, "solid IoU   {:6.2}%  (orig {}, recon {}, both {})", 100.0 * self.solid_iou(), self.solid.original, self.solid.reconstructed, self.solid.both)?;
        writeln!(f, "surface     {}  (n={})", pct(self.surface.hits, self.surface.total), self.surface.total)?;
        writeln!(f, "biome cells {}  (n={})", pct(self.biomes.hits, self.biomes.total), self.biomes.total)?;
        if !self.confusions.is_empty() {
            writeln!(f, "top confusions (original -> reconstructed):")?;
            for c in &self.confusions {
                writeln!(f, "  {:>9}  {} -> {}", c.count, c.original, c.reconstructed)?;
            }
        }
        Ok(())
    }
}
