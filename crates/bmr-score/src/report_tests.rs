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
    let got: Vec<_> = r.confusions.iter().map(|c| (c.original.as_str(), c.reconstructed.as_str(), c.count)).collect();
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
