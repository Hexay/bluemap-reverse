//! Material pass 2: a gap under a build copies, per height, the nearest column that knows its block there
//! (a pass-1 fill or an observed full cube), else `material::default_at`.

use rustc_hash::FxHashMap;

use bmr_world::BlockState;

use crate::columns::{Column, ring};
use crate::material::{Run, SolidGap};

/// How far a gap searches for a column to copy (Chebyshev radius).
const COPY_RADIUS: i32 = 16;

/// What pass 2 copies from: per column, pass-1 runs and observed full cubes (sorted by y).
pub struct Sources<'r, 'a> {
    resolved: &'r FxHashMap<Column, Vec<(i32, i32, BlockState)>>,
    observed: FxHashMap<Column, Vec<(i32, &'a BlockState)>>,
    /// Ring offsets, nearest ring first, in `ring` order within each ring.
    offsets: Vec<(i32, i32)>,
}

impl<'r, 'a> Sources<'r, 'a> {
    pub fn new(
        resolved: &'r FxHashMap<Column, Vec<(i32, i32, BlockState)>>,
        full_cubes: impl IntoIterator<Item = ((i32, i32, i32), &'a BlockState)>,
    ) -> Self {
        let mut observed: FxHashMap<Column, Vec<(i32, &BlockState)>> = FxHashMap::default();
        for ((x, y, z), s) in full_cubes {
            observed.entry((x, z)).or_default().push((y, s));
        }
        observed.values_mut().for_each(|v| v.sort_unstable_by_key(|&(y, _)| y));
        let offsets = (1..=COPY_RADIUS).flat_map(ring).collect();
        Self { resolved, observed, offsets }
    }
}

enum Pick<'a> {
    Copied(&'a BlockState),
    Default(&'a str),
}

/// Per y, the first column in ring order that knows it wins: each column is visited once and claims every
/// still-open height it covers (was a full ring search per cell, the fill's hottest loop).
pub fn copy_nearest<'d>(
    g: &SolidGap,
    src: &Sources,
    default: impl Fn(i32) -> &'d str,
    named: &impl Fn(&str) -> BlockState,
) -> Vec<Run> {
    let (cx, cz) = g.column;
    let mut picks: Vec<Option<Pick>> = (g.ylo..=g.yhi).map(|_| None).collect();
    let mut open = picks.len();
    let mut claim = |y: i32, s, open: &mut usize| {
        let slot = &mut picks[(y - g.ylo) as usize];
        if slot.is_none() {
            *slot = Some(Pick::Copied(s));
            *open -= 1;
        }
    };
    for &(dx, dz) in &src.offsets {
        if open == 0 {
            break;
        }
        let col = (cx + dx, cz + dz);
        for (lo, hi, s) in src.resolved.get(&col).into_iter().flatten() {
            for y in (*lo).max(g.ylo)..=(*hi).min(g.yhi) {
                claim(y, s, &mut open);
            }
        }
        if let Some(obs) = src.observed.get(&col) {
            let from = obs.partition_point(|&(y, _)| y < g.ylo);
            for &(y, s) in obs[from..].iter().take_while(|&&(y, _)| y <= g.yhi) {
                claim(y, s, &mut open);
            }
        }
    }

    let picks: Vec<Pick> = (g.ylo..)
        .zip(picks)
        .map(|(y, p)| p.unwrap_or_else(|| Pick::Default(default(y))))
        .collect();
    let mut out: Vec<Run> = Vec::new();
    let mut last: Option<&Pick> = None;
    for y in (g.ylo..=g.yhi).rev() {
        let pick = &picks[(y - g.ylo) as usize];
        let same = match (last, pick) {
            (Some(Pick::Copied(a)), Pick::Copied(b)) => a == b,
            (Some(Pick::Default(a)), Pick::Default(b)) => a == b,
            _ => false,
        };
        match out.last_mut() {
            Some(r) if same => r.ylo = y,
            _ => {
                let state = match pick {
                    Pick::Copied(s) => (*s).clone(),
                    Pick::Default(name) => named(name),
                };
                out.push(Run { column: g.column, ylo: y, yhi: y, state });
            }
        }
        last = Some(pick);
    }
    out
}
