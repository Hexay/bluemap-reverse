//! Observed cell faces → most likely block state from the library.
//! `candidates` (cacheable per signature) returns every equally good entry; `resolve` picks one using the
//! observed tint (redstone power) and closeness to the default state.

use std::collections::BTreeSet;

use crate::face::{FaceKey, Tex, close, corner_aligned, normalized};
use crate::library::Library;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum How {
    /// Observed faces equal a full library signature.
    Exact,
    /// Equal after undoing BlueMap's random x/z offset.
    Offset,
    /// Best subset match: missing faces are on the cell boundary (culled by neighbours).
    Partial,
}

#[derive(Debug, Clone)]
pub struct Candidates {
    pub ids: Vec<usize>,
    pub how: How,
}

#[derive(Default)]
struct Fit {
    matched: u32,
    extra: u32,
    missing_interior: u32,
    missing_boundary: u32,
}

impl Fit {
    fn acceptable(&self) -> bool {
        self.matched > 0 && self.extra == 0 && self.missing_interior == 0
    }
}

pub fn candidates(lib: &Library, sig: &[FaceKey]) -> Option<Candidates> {
    if let Some(ids) = lib.exact(sig) {
        return Some(Candidates { ids: ids.to_vec(), how: How::Exact });
    }
    if let Some(ids) = lib.exact_normalized(&normalized(sig)) {
        return Some(Candidates { ids: ids.to_vec(), how: How::Offset });
    }
    let textures: BTreeSet<Tex> = sig.iter().map(|k| k.texture).collect();
    if sig.len() <= MAX_FUZZY_QUADS {
        let aligned = corner_aligned(sig);
        let ids: Vec<usize> = lib
            .with_textures(textures.iter().copied())
            .into_iter()
            .filter(|&id| lib.entries[id].sig.len() == sig.len() && all_close(&aligned, &corner_aligned(&lib.entries[id].sig)))
            .collect();
        if !ids.is_empty() {
            return Some(Candidates { ids, how: How::Offset });
        }
    }
    let mut best_missing = u32::MAX;
    let mut ids = Vec::new();
    for id in lib.with_textures(textures) {
        let fit = fit(sig, &lib.entries[id].sig);
        if !fit.acceptable() || fit.missing_boundary > best_missing {
            continue;
        }
        if fit.missing_boundary < best_missing {
            best_missing = fit.missing_boundary;
            ids.clear();
        }
        ids.push(id);
    }
    (!ids.is_empty()).then_some(Candidates { ids, how: How::Partial })
}

/// Pick among equally good candidates: nearest library tint when tints tell them apart, then fewest
/// properties off the default state, then lowest id.
pub fn resolve(lib: &Library, ids: &[usize], tint: Option<[u8; 3]>) -> usize {
    let tints: BTreeSet<[u8; 3]> = ids.iter().filter_map(|&id| lib.entries[id].tint).collect();
    let by_tint = tint.filter(|_| tints.len() > 1);
    *ids.iter()
        .min_by_key(|&&id| {
            let e = &lib.entries[id];
            let tint_err = match (by_tint, e.tint) {
                (Some(o), Some(t)) => (0..3).map(|a| (o[a] as i32 - t[a] as i32).pow(2)).sum::<i32>(),
                _ => 0,
            };
            (tint_err, e.default_distance, id)
        })
        .expect("non-empty candidates")
}

/// Offset jitter only matters for small plant-like models; bigger signatures skip the O(n²) pairing.
const MAX_FUZZY_QUADS: usize = 16;

/// Every key in `a` pairs with a distinct close key in `b` (same length assumed).
fn all_close(a: &[FaceKey], b: &[FaceKey]) -> bool {
    let mut used = vec![false; b.len()];
    a.iter().all(|k| match (0..b.len()).find(|&j| !used[j] && close(k, &b[j])) {
        Some(j) => {
            used[j] = true;
            true
        }
        None => false,
    })
}

/// Merge two sorted multisets.
fn fit(observed: &[FaceKey], sig: &[FaceKey]) -> Fit {
    let (mut i, mut j, mut f) = (0, 0, Fit::default());
    while i < observed.len() || j < sig.len() {
        match (observed.get(i), sig.get(j)) {
            (Some(o), Some(s)) if o == s => {
                f.matched += 1;
                i += 1;
                j += 1;
            }
            (Some(o), Some(s)) if o < s => {
                f.extra += 1;
                i += 1;
            }
            (Some(_), None) => {
                f.extra += 1;
                i += 1;
            }
            (_, Some(s)) => {
                if s.on_boundary() { f.missing_boundary += 1 } else { f.missing_interior += 1 }
                j += 1;
            }
            (None, None) => unreachable!(),
        }
    }
    f
}
