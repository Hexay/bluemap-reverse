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

/// See docs/plan.md "Scoring". `rendered` = cells BlueMap drew faces for (needs the mirror): the
/// ceiling for pure inversion. `exposed` = non-air with an air neighbour, which also counts dark cave
/// walls BlueMap never draws.
#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub chunks: u64,
    pub partial_chunks_skipped: u64,
    pub columns: u64,
    /// Every voxel; dominated by air, so it flatters an empty reconstruction.
    pub all: Accuracy,
    /// Voxels non-air in either world — the headline number.
    pub occupied: Accuracy,
    pub rendered: Accuracy,
    pub exposed: Accuracy,
    /// Non-air voxels.
    pub solid: Overlap,
    /// Topmost non-air block per column: same y and same state.
    pub surface: Hits,
    /// 4×4×4 biome cells with the same biome.
    pub biomes: Hits,
    pub confusions: Vec<Confusion>,
    /// Confusions among rendered cells only: pure inversion errors.
    pub rendered_confusions: Vec<Confusion>,
    #[serde(skip)]
    pub(crate) confusion_counts: HashMap<(String, String), u64>,
    #[serde(skip)]
    pub(crate) rendered_confusion_counts: HashMap<(String, String), u64>,
}

impl Report {
    pub(crate) fn merge(&mut self, o: Report) {
        self.chunks += o.chunks;
        self.partial_chunks_skipped += o.partial_chunks_skipped;
        self.columns += o.columns;
        for (a, b) in [
            (&mut self.all, o.all),
            (&mut self.occupied, o.occupied),
            (&mut self.rendered, o.rendered),
            (&mut self.exposed, o.exposed),
        ] {
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
        for (k, v) in o.rendered_confusion_counts {
            *self.rendered_confusion_counts.entry(k).or_default() += v;
        }
    }

    pub(crate) fn finish(&mut self, top: usize) {
        self.confusions = top_confusions(&mut self.confusion_counts, top);
        self.rendered_confusions = top_confusions(&mut self.rendered_confusion_counts, top);
    }

    pub fn solid_iou(&self) -> f64 {
        ratio(self.solid.both, self.solid.original + self.solid.reconstructed - self.solid.both)
    }
}

fn top_confusions(counts: &mut HashMap<(String, String), u64>, top: usize) -> Vec<Confusion> {
    let mut v: Vec<_> = counts.drain().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.into_iter()
        .take(top)
        .map(|((original, reconstructed), count)| Confusion { original, reconstructed, count })
        .collect()
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
        for (label, a) in [
            ("occupied  ", self.occupied),
            ("rendered  ", self.rendered),
            ("exposed   ", self.exposed),
            ("all voxels", self.all),
        ] {
            if a.total == 0 {
                continue;
            }
            writeln!(f, "{label}  state {}  name {}  (n={})", pct(a.exact, a.total), pct(a.name, a.total), a.total)?;
        }
        writeln!(f, "solid IoU   {:6.2}%  (orig {}, recon {}, both {})", 100.0 * self.solid_iou(), self.solid.original, self.solid.reconstructed, self.solid.both)?;
        writeln!(f, "surface     {}  (n={})", pct(self.surface.hits, self.surface.total), self.surface.total)?;
        writeln!(f, "biome cells {}  (n={})", pct(self.biomes.hits, self.biomes.total), self.biomes.total)?;
        for (title, list) in [
            ("top confusions (original -> reconstructed)", &self.confusions),
            ("rendered-cell confusions", &self.rendered_confusions),
        ] {
            if list.is_empty() {
                continue;
            }
            writeln!(f, "{title}:")?;
            for c in list {
                writeln!(f, "  {:>9}  {} -> {}", c.count, c.original, c.reconstructed)?;
            }
        }
        Ok(())
    }
}
