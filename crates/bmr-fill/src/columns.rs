//! Vertical runs of unobserved cells ("gaps") between observed blocks, classified from face evidence.
//! Streaming per column instead of a 3D flood: memory scales with columns, not voxels.
//! A gap is one connected unseen volume vertically, so its evidence applies to all of it:
//! liquid evidence without solid → liquid; solid evidence outweighing open → solid; else air.
//! Cave caveat: below `remove-caves-below-y`, dark air also loses faces, so dark caves read as solid.

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
                let rock_under = parts.iter().find(|(_, hi)| p.mask.is_some_and(|(mlo, _)| *hi == mlo - 1)).is_none_or(
                    |&(lo, hi)| classify(ce, lo, hi) == Fill::Solid,
                );
                for (ylo, yhi) in parts {
                    let floored = ylo > p.min_y || y >= p.min_y;
                    let mut push = |ylo: i32, yhi: i32, fill| {
                        if ylo <= yhi {
                            out.push(Gap { column: col, ylo, yhi, fill, floored });
                        }
                    };
                    if p.masked(ylo) {
                        // left out of the render (nether roof): its drawn faces say nothing
                        let roof = if rock_under { ylo } else { p.roof_from.max(ylo) };
                        push(ylo, roof - 1, Fill::Air);
                        push(roof, yhi, Fill::Solid);
                    } else if yhi == p.max_y {
                        // open to the sky: anything in it would show its top face
                        push(ylo, yhi, Fill::Air);
                    } else {
                        push(ylo, yhi, classify(ce, ylo, yhi));
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
