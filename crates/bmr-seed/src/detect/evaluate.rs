//! Detections vs known structure starts (tools/structure_truth.py output), per structure set.

use std::collections::BTreeMap;

use serde::Serialize;

use super::Detection;
use crate::observation::Observation;

#[derive(Debug, Default, Serialize)]
pub struct SetScore {
    /// truth starts inside the scanned area
    pub truth: usize,
    pub detections: usize,
    /// detections whose candidate chunks include a true start
    pub correct: usize,
    /// detections near a true start (≤ 4 chunks) whose candidates miss it: a wrong corner rule
    pub misplaced: usize,
    /// detections with no true start nearby
    pub false_positives: usize,
    /// average candidate chunks per correct detection
    pub options: f64,
}

/// `in_area` decides which truth starts could have been seen (e.g. inside the rendered chunks).
pub fn evaluate(detections: &[Detection], truth: &[Observation], in_area: impl Fn([i32; 2]) -> bool) -> BTreeMap<String, SetScore> {
    let mut scores: BTreeMap<String, SetScore> = BTreeMap::new();
    for t in truth.iter().filter(|t| in_area(t.chunks[0])) {
        scores.entry(t.set.clone()).or_default().truth += 1;
    }
    for d in detections {
        let s = scores.entry(d.set.to_string()).or_default();
        s.detections += 1;
        let starts = truth.iter().filter(|t| t.set == d.set).map(|t| t.chunks[0]);
        let center = [(d.min[0] + d.max[0]).div_euclid(32), (d.min[2] + d.max[2]).div_euclid(32)];
        let near: Vec<[i32; 2]> = starts.filter(|c| (c[0] - center[0]).abs() <= 4 && (c[1] - center[1]).abs() <= 4).collect();
        if near.iter().any(|c| d.chunks.contains(c)) {
            s.correct += 1;
            s.options += d.chunks.len() as f64;
        } else if near.is_empty() {
            s.false_positives += 1;
        } else {
            s.misplaced += 1;
        }
    }
    for s in scores.values_mut() {
        s.options /= s.correct.max(1) as f64;
    }
    scores
}
