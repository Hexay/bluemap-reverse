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

/// See docs/architecture.md "Scoring". `rendered` = cells BlueMap drew faces for (needs the mirror): the
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
    /// Rendered cells reconstructed exactly or as a look-alike (needs a pack): the achievable ceiling is 100%.
    pub rendered_alike: u64,
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
    /// Biome cells: (original, reconstructed) most frequent first.
    pub biome_confusions: Vec<Confusion>,
    /// Positions of the confusion requested via `Scope::sample`.
    pub samples: Vec<(i32, i32, i32)>,
    #[serde(skip)]
    pub(crate) confusion_counts: HashMap<(String, String), u64>,
    #[serde(skip)]
    pub(crate) rendered_confusion_counts: HashMap<(String, String), u64>,
    #[serde(skip)]
    pub(crate) biome_confusion_counts: HashMap<(String, String), u64>,
}

impl Report {
    pub(crate) fn merge(&mut self, o: Report) {
        self.chunks += o.chunks;
        self.partial_chunks_skipped += o.partial_chunks_skipped;
        self.columns += o.columns;
        self.rendered_alike += o.rendered_alike;
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
        for (k, v) in o.biome_confusion_counts {
            *self.biome_confusion_counts.entry(k).or_default() += v;
        }
        self.samples.extend(o.samples);
        self.samples.sort();
        self.samples.truncate(20);
    }

    pub(crate) fn finish(&mut self, top: usize) {
        self.confusions = top_confusions(&mut self.confusion_counts, top);
        self.rendered_confusions = top_confusions(&mut self.rendered_confusion_counts, top);
        self.biome_confusions = top_confusions(&mut self.biome_confusion_counts, top);
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
        writeln!(
            f,
            "chunks {} ({} partial skipped), columns {}",
            self.chunks, self.partial_chunks_skipped, self.columns
        )?;
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
        if self.rendered_alike > self.rendered.exact {
            writeln!(
                f,
                "rendered    state {}  counting look-alikes as correct",
                pct(self.rendered_alike, self.rendered.total)
            )?;
        }
        writeln!(
            f,
            "solid IoU   {:6.2}%  (orig {}, recon {}, both {})",
            100.0 * self.solid_iou(),
            self.solid.original,
            self.solid.reconstructed,
            self.solid.both
        )?;
        writeln!(f, "surface     {}  (n={})", pct(self.surface.hits, self.surface.total), self.surface.total)?;
        writeln!(f, "biome cells {}  (n={})", pct(self.biomes.hits, self.biomes.total), self.biomes.total)?;
        for (title, list) in [
            ("top confusions (original -> reconstructed)", &self.confusions),
            ("rendered-cell confusions", &self.rendered_confusions),
            ("biome confusions (4x4x4 cells)", &self.biome_confusions),
        ] {
            if list.is_empty() {
                continue;
            }
            writeln!(f, "{title}:")?;
            for c in list {
                writeln!(f, "  {:>9}  {} -> {}", c.count, c.original, c.reconstructed)?;
            }
        }
        if !self.samples.is_empty() {
            writeln!(f, "samples: {:?}", self.samples)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(a: &str, b: &str) -> (String, String) {
        (a.into(), b.into())
    }

    fn part(seed: u64) -> Report {
        let mut r = Report {
            chunks: 1,
            columns: 2 * seed,
            all: Accuracy { total: 10 * seed, exact: 8 * seed, name: 9 * seed },
            occupied: Accuracy { total: 4, exact: 2, name: 3 },
            solid: Overlap { original: 3, reconstructed: seed, both: 1 },
            surface: Hits { total: 2, hits: 1 },
            samples: (0..15).map(|i| (i * seed as i32, 0, 0)).collect(),
            ..Report::default()
        };
        r.confusion_counts.insert(pair("a", "b"), seed);
        r.confusion_counts.insert(pair("c", "d"), 1);
        r.biome_confusion_counts.insert(pair("plains", "desert"), seed);
        r
    }

    #[test]
    fn merge_sums_every_counter() {
        let mut r = Report::default();
        r.merge(part(1));
        r.merge(part(3));
        assert_eq!((r.chunks, r.columns), (2, 8));
        assert_eq!((r.all.total, r.all.exact, r.all.name), (40, 32, 36));
        assert_eq!((r.occupied.total, r.occupied.exact, r.occupied.name), (8, 4, 6));
        assert_eq!((r.solid.original, r.solid.reconstructed, r.solid.both), (6, 4, 2));
        assert_eq!((r.surface.total, r.surface.hits), (4, 2));
        assert_eq!(r.confusion_counts[&pair("a", "b")], 4);
        assert_eq!(r.confusion_counts[&pair("c", "d")], 2);
        assert_eq!(r.biome_confusion_counts[&pair("plains", "desert")], 4);
    }

    #[test]
    fn merged_samples_stay_sorted_and_capped() {
        let mut r = Report::default();
        r.merge(part(3));
        r.merge(part(1));
        assert_eq!(r.samples.len(), 20);
        assert!(r.samples.is_sorted());
        assert_eq!(r.samples[..3], [(0, 0, 0), (0, 0, 0), (1, 0, 0)]);
    }

    #[test]
    fn finish_ranks_by_count_then_label() {
        let mut r = Report::default();
        for (k, n) in [(pair("x", "y"), 2), (pair("b", "a"), 5), (pair("a", "z"), 2), (pair("q", "r"), 1)] {
            r.confusion_counts.insert(k, n);
        }
        r.finish(3);
        let got: Vec<_> =
            r.confusions.iter().map(|c| (c.original.as_str(), c.reconstructed.as_str(), c.count)).collect();
        assert_eq!(got, [("b", "a", 5), ("a", "z", 2), ("x", "y", 2)]);
        assert!(r.confusion_counts.is_empty(), "finish drains the counts");
    }

    #[test]
    fn empty_report_ratios_are_perfect() {
        let r = Report::default();
        assert_eq!(r.solid_iou(), 1.0);
        let text = r.to_string();
        assert!(text.contains("solid IoU   100.00%"), "{text}");
        assert!(!text.contains("occupied"), "zero-total accuracies are omitted");
    }
}
