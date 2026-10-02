//! Observed cell faces → most likely block state from the library.
//! `candidates` (cacheable per signature) returns every equally good entry; `resolve` picks one using the
//! observed tint (redstone power) and closeness to the default state.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use rustc_hash::FxHashMap;

use crate::face::{CellFaces, FaceKey, Tex, Uv, close, corner_aligned, normalized};
use crate::library::{Entry, Library};
use crate::lookalike::{light_decides, uv_decides};

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
    /// Interior but axis-aligned: culled when the model gives it a cullface (a candle's base on a cake).
    missing_cullable: u32,
    missing_boundary: u32,
}

impl Fit {
    /// Strict: only boundary faces may be missing. Relaxed: also cullable ones, while most of the model shows
    /// (a cell holding only a neighbour's overhang must not pass as that neighbour's block).
    fn missing(&self, strict: bool) -> Option<u32> {
        let ok = self.matched > 0 && self.extra == 0 && self.missing_interior == 0;
        match strict {
            true => (ok && self.missing_cullable == 0).then_some(self.missing_boundary),
            false => {
                (ok && self.missing_cullable < self.matched).then_some(self.missing_boundary + self.missing_cullable)
            }
        }
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
            .filter(|&id| {
                lib.entries[id].sig.len() == sig.len() && all_close(&aligned, &corner_aligned(&lib.entries[id].sig))
            })
            .collect();
        if !ids.is_empty() {
            return Some(Candidates { ids, how: How::Offset });
        }
    }
    partial(lib, sig, textures, true)
}

/// Last resort for cells nothing else explains (run after overhang crediting): a partial match that may
/// also miss interior axis-aligned faces, which a model's cullface hides next to a solid neighbour.
pub fn candidates_cullable(lib: &Library, sig: &[FaceKey]) -> Option<Candidates> {
    partial(lib, sig, sig.iter().map(|k| k.texture).collect(), false)
}

fn partial(lib: &Library, sig: &[FaceKey], textures: BTreeSet<Tex>, strict: bool) -> Option<Candidates> {
    let missing: Vec<(usize, u32)> = lib
        .with_textures(textures)
        .into_iter()
        .filter_map(|id| fit(sig, &lib.entries[id].sig).missing(strict).map(|m| (id, m)))
        .collect();
    let best = missing.iter().map(|&(_, m)| m).min()?;
    let ids = missing.into_iter().filter(|&(_, m)| m == best).map(|(id, _)| id).collect();
    Some(Candidates { ids, how: How::Partial })
}

/// Pick among equally good candidates (same faces): nearest library tint when tints tell them apart, then
/// most faces whose UVs agree (door hinges, glazed terracotta), then nearest own light (lit ores), each
/// only where it decides (see lookalike.rs), then fewest properties off the default state, then lowest id.
pub fn resolve(lib: &Library, ids: &[usize], obs: &CellFaces) -> usize {
    if let [id] = ids {
        return *id;
    }
    let tints: BTreeSet<[u8; 3]> = ids.iter().filter_map(|&id| lib.entries[id].tint).collect();
    let by_tint = obs.tint().filter(|_| tints.len() > 1);
    let mut seen: FxHashMap<(FaceKey, Uv), u32> = FxHashMap::default();
    if uv_decides(lib, ids) {
        for &p in &obs.uvs {
            *seen.entry(p).or_default() += 1;
        }
    }
    let light = light_decides(lib, ids).then(|| obs.light());
    *ids.iter()
        .min_by_key(|&&id| {
            let e = &lib.entries[id];
            let tint_err = match (by_tint, e.tint) {
                (Some(o), Some(t)) => (0..3).map(|a| (o[a] as i32 - t[a] as i32).pow(2)).sum::<i32>(),
                _ => 0,
            };
            let uv = if seen.is_empty() { 0 } else { uv_agreement(e, &seen) };
            (tint_err, Reverse(uv), light.map_or(0, |l| l.abs_diff(e.light)), e.default_distance, id)
        })
        .expect("non-empty candidates")
}

/// Faces of `e` observed with the same UVs (multiset intersection).
fn uv_agreement(e: &Entry, seen: &FxHashMap<(FaceKey, Uv), u32>) -> u32 {
    let mut used: FxHashMap<(FaceKey, Uv), u32> = FxHashMap::default();
    let mut fits = |p: (FaceKey, Uv)| {
        let n = used.entry(p).or_default();
        *n += 1;
        *n <= seen.get(&p).copied().unwrap_or(0)
    };
    e.sig.iter().zip(&e.uvs).map(|(k, uv)| (*k, *uv)).filter(|&p| fits(p)).count() as u32
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
                match (s.on_boundary(), s.axis_aligned()) {
                    (true, _) => f.missing_boundary += 1,
                    (false, true) => f.missing_cullable += 1,
                    (false, false) => f.missing_interior += 1,
                }
                j += 1;
            }
            (None, None) => unreachable!(),
        }
    }
    f
}
