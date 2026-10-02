//! Vertical runs of unobserved cells ("gaps") between observed blocks, classified from face evidence.
//! Streaming per column instead of a 3D flood: memory scales with columns, not voxels.
//! A gap is one connected unseen volume vertically, so its evidence applies to all of it:
//! liquid evidence without solid → liquid; solid evidence outweighing open → solid; else air.
//! Cave caveat: below `remove-caves-below-y`, dark air also loses faces, so dark caves read as solid
//! (`dark_below`).

use bmr_invert::evidence::Evidence;
use bmr_invert::face::Liquid;

use crate::profile::Profile;
use rustc_hash::FxHashMap;

pub type Column = (i32, i32);

pub struct Bounds {
    /// Every rendered column.
    pub columns: Vec<Column>,
    pub profile: Profile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Solid,
    Liquid(Liquid),
    /// Open/sky evidence, or none: air unless a regeneration says otherwise below the cave cut-off.
    Air,
}

pub struct Gap {
    pub column: Column,
    pub ylo: i32,
    pub yhi: i32,
    pub fill: Fill,
    /// An observed block lies directly below (false: the gap runs to the world floor).
    pub floored: bool,
}

/// Evidence of one column, y sorted descending (the fill walks columns top-down).
#[derive(Default)]
pub struct ColumnEvidence {
    pub solid: Vec<i32>,
    pub liquid: Vec<(i32, Liquid)>,
    pub open: Vec<i32>,
}

/// Membership test for a descending walk over a descending-sorted list: amortised O(1) per query.
pub struct Cursor<'a> {
    ys: &'a [i32],
    i: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(ys: &'a [i32]) -> Self {
        Self { ys, i: 0 }
    }

    /// Queries must come in non-increasing y.
    pub fn hit(&mut self, y: i32) -> bool {
        while self.i < self.ys.len() && self.ys[self.i] > y {
            self.i += 1;
        }
        self.i < self.ys.len() && self.ys[self.i] == y
    }
}

pub type EvidenceByColumn = FxHashMap<Column, ColumnEvidence>;

pub fn evidence_by_column(ev: &Evidence) -> EvidenceByColumn {
    let mut by_col: EvidenceByColumn = FxHashMap::default();
    for &(x, y, z) in &ev.solid {
        by_col.entry((x, z)).or_default().solid.push(y);
    }
    for (&(x, y, z), &l) in &ev.liquid {
        by_col.entry((x, z)).or_default().liquid.push((y, l));
    }
    for &(x, y, z) in &ev.open {
        by_col.entry((x, z)).or_default().open.push(y);
    }
    for ce in by_col.values_mut() {
        ce.solid.sort_unstable_by(|a, b| b.cmp(a));
        ce.open.sort_unstable_by(|a, b| b.cmp(a));
        ce.liquid.sort_unstable_by_key(|l| std::cmp::Reverse(l.0));
    }
    by_col
}

/// `observed_ys`: per column, y of every observed cell (any order).
pub fn gaps(observed_ys: &FxHashMap<Column, Vec<i32>>, by_col: &EvidenceByColumn, bounds: &Bounds) -> Vec<Gap> {
    let mut out = Vec::new();
    let empty = ColumnEvidence::default();
    let p = &bounds.profile;
    for &col in &bounds.columns {
        let mut ys = observed_ys.get(&col).cloned().unwrap_or_default();
        ys.sort_unstable_by(|a, b| b.cmp(a));
        let ce = by_col.get(&col).unwrap_or(&empty);
        let mut cursor = p.max_y;
        for y in ys.into_iter().chain([p.min_y - 1]) {
            if y < cursor {
                let parts = split_at_mask(y + 1, cursor, p.mask);
                // rock just under the mask carries on into it; open space goes on up to the roof's underside
                let rock_under = parts
                    .iter()
                    .find(|(_, hi)| p.mask.is_some_and(|(mlo, _)| *hi == mlo - 1))
                    .is_none_or(|&(lo, hi)| classify(ce, lo, hi) == Fill::Solid);
                for (ylo, yhi) in parts {
                    let floored = ylo > p.min_y || y >= p.min_y;
                    let mut push = |ylo: i32, yhi: i32, fill, floored| {
                        if ylo <= yhi {
                            out.push(Gap { column: col, ylo, yhi, fill, floored });
                        }
                    };
                    if p.masked(ylo) {
                        // left out of the render (nether roof): its drawn faces say nothing
                        let roof = if rock_under { ylo } else { p.roof_from.max(ylo) };
                        push(ylo, roof - 1, Fill::Air, floored);
                        push(roof, yhi, Fill::Solid, floored);
                    } else if yhi == p.max_y {
                        // open to the sky: anything in it would show its top face
                        push(ylo, yhi, Fill::Air, floored);
                    } else {
                        for (lo, hi, fill) in layers(ce, ylo, yhi, p.cave_y) {
                            push(lo, hi, fill, floored || lo > ylo);
                        }
                    }
                }
            }
            cursor = cursor.min(y - 1);
        }
    }
    out
}

/// [ylo, yhi] cut into the parts below, inside and above `mask`, top-down.
fn split_at_mask(ylo: i32, yhi: i32, mask: Option<(i32, i32)>) -> Vec<(i32, i32)> {
    let Some((mlo, mhi)) = mask else { return vec![(ylo, yhi)] };
    [(mhi + 1, yhi), (mlo.max(ylo), mhi.min(yhi)), (ylo, mlo - 1)]
        .into_iter()
        .map(|(lo, hi)| (lo.max(ylo), hi.min(yhi)))
        .filter(|(lo, hi)| lo <= hi)
        .collect()
}

/// Column offsets on the square ring at Chebyshev distance `r`.
pub fn ring(r: i32) -> impl Iterator<Item = (i32, i32)> {
    (-r..=r).flat_map(move |dx| (-r..=r).map(move |dz| (dx, dz))).filter(move |(dx, dz)| dx.abs().max(dz.abs()) == r)
}

/// One run of same-kind evidence down a gap: solid, or open/liquid (`liquid` set if any of it is liquid).
struct Run {
    top: i32,
    bottom: i32,
    solid: bool,
    liquid: Option<Liquid>,
}

/// [ylo, yhi] cut into layers, top-down, one per run of solid or open/liquid evidence; a single vote over the
/// whole gap let a flooded ravine's floor outvote the water drawn beside it (render round-trip, 2026-10-02).
/// Open space reaches down to its lowest evidence and rock fills the unknown cells between runs. A bottom
/// liquid layer runs on to `ylo` (unfloored: `liquid.rs` estimates the sea floor). A bottom air layer goes on
/// to `ylo` only above `cave_y`: below it BlueMap drops unlit faces, so cells under the last
/// sign of an opening are unknown, and rock is far likelier (leaving them air carved shafts down to bedrock);
/// above it nothing is culled, so open space with nothing drawn under it is real (an island over void).
fn layers(ce: &ColumnEvidence, ylo: i32, yhi: i32, cave_y: i32) -> Vec<(i32, i32, Fill)> {
    let inside = |y: &i32| (ylo..=yhi).contains(y);
    let mut points: Vec<(i32, bool, Option<Liquid>)> =
        ce.solid.iter().filter(|y| inside(y)).map(|&y| (y, true, None)).collect();
    points.extend(ce.open.iter().filter(|y| inside(y)).map(|&y| (y, false, None)));
    points.extend(ce.liquid.iter().filter(|(y, _)| inside(y)).map(|&(y, l)| (y, false, Some(l))));
    points.sort_unstable_by_key(|&(y, _, _)| std::cmp::Reverse(y));
    // one point per cell; solid wins a cell it shares with liquid evidence (a missing liquid face means liquid
    // or full block), and liquid evidence names the liquid of an open cell
    let mut cells: Vec<(i32, bool, Option<Liquid>)> = Vec::with_capacity(points.len());
    for (y, solid, liquid) in points {
        match cells.last_mut() {
            Some(c) if c.0 == y => {
                c.1 |= solid;
                c.2 = c.2.or(liquid);
            }
            _ => cells.push((y, solid, liquid)),
        }
    }

    let mut runs: Vec<Run> = Vec::new();
    for (y, solid, liquid) in cells {
        match runs.last_mut() {
            Some(r) if r.solid == solid => {
                r.bottom = y;
                r.liquid = r.liquid.or(liquid);
            }
            _ => runs.push(Run { top: y, bottom: y, solid, liquid }),
        }
    }
    let fill = |r: &Run| match (r.solid, r.liquid) {
        (true, _) => Fill::Solid,
        (false, Some(l)) => Fill::Liquid(l),
        (false, None) => Fill::Air,
    };
    if runs.is_empty() {
        return vec![(ylo, yhi, Fill::Air)];
    }
    let mut out = Vec::new();
    let mut hi = yhi;
    for (i, r) in runs.iter().enumerate() {
        let lo = match runs.get(i + 1) {
            Some(next) if r.solid => next.top + 1,
            Some(_) => r.bottom,
            None if fill(r) == Fill::Air && r.bottom < cave_y => r.bottom.max(ylo),
            None => ylo,
        };
        out.push((lo, hi, fill(r)));
        hi = lo - 1;
    }
    if hi >= ylo {
        out.push((ylo, hi, Fill::Solid));
    }
    out
}

fn classify(ce: &ColumnEvidence, ylo: i32, yhi: i32) -> Fill {
    let inside = |y: &i32| (ylo..=yhi).contains(y);
    let solid = ce.solid.iter().filter(|y| inside(y)).count();
    let open = ce.open.iter().filter(|y| inside(y)).count();
    let liquid = ce.liquid.iter().find(|(y, _)| inside(y)).map(|&(_, l)| l);
    match liquid {
        Some(l) if solid == 0 => Fill::Liquid(l),
        _ if solid > 0 && solid >= open => Fill::Solid,
        _ => Fill::Air,
    }
}

#[cfg(test)]
#[path = "columns_tests.rs"]
mod tests;
