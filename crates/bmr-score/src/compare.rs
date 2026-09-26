//! Voxel-by-voxel comparison of an original world against a reconstruction, one region per task.
//! All air variants compare equal (BlueMap cannot tell them apart).

use rustc_hash::{FxHashMap, FxHashSet};

use anyhow::Result;
use bmr_world::{BlockState, Chunk, ChunkPos, World};
use rayon::prelude::*;

use crate::report::Report;

/// World (x, z) → whether the column is scored (e.g. inside the rendered area).
pub type ColumnFilter<'a> = &'a (dyn Fn(i32, i32) -> bool + Sync);

pub type Cell = (i32, i32, i32);

pub struct Scope<'a> {
    pub columns: ColumnFilter<'a>,
    /// Cells BlueMap drew faces for; enables the `rendered` metric.
    pub rendered: Option<&'a FxHashSet<Cell>>,
    /// (original, reconstructed) labels whose positions to sample into `Report::samples`.
    pub sample: Option<(&'a str, &'a str)>,
    /// State label → look-alike group (states BlueMap renders identically); enables `rendered_alike`.
    pub lookalikes: Option<&'a FxHashMap<String, u32>>,
}

const MAX_SAMPLES: usize = 20;

pub fn score(original: &World, reconstructed: &World, scope: &Scope, top_confusions: usize) -> Result<Report> {
    let regions = original.regions()?;
    let parts = regions
        .par_iter()
        .map(|&r| score_region(original, reconstructed, r, scope))
        .collect::<Result<Vec<_>>>()?;
    let mut report = Report::default();
    for p in parts {
        report.merge(p);
    }
    report.finish(top_confusions);
    Ok(report)
}

type Chunks = FxHashMap<ChunkPos, Chunk>;

fn score_region(original: &World, reconstructed: &World, region: (i32, i32), scope: &Scope) -> Result<Report> {
    let filter = scope.columns;
    let orig = original.read_region(region)?;
    let recon = reconstructed.read_region(region)?;
    let mut rep = Report::default();
    for (&pos, chunk) in &orig {
        if !chunk.is_full() {
            rep.partial_chunks_skipped += 1;
            continue;
        }
        let Some(y_range) = chunk.y_range() else { continue };
        let other = recon.get(&pos);
        let mut scored = false;
        for lz in 0..16 {
            for lx in 0..16 {
                if filter(pos.0 * 16 + lx as i32, pos.1 * 16 + lz as i32) {
                    scored = true;
                    rep.columns += 1;
                    score_column(&mut rep, &orig, pos, chunk, other, lx, lz, y_range, scope);
                }
            }
        }
        if scored {
            rep.chunks += 1;
            score_biomes(&mut rep, pos, chunk, other, filter);
        }
    }
    Ok(rep)
}

#[allow(clippy::too_many_arguments)]
fn score_column(
    rep: &mut Report,
    orig: &Chunks,
    pos: ChunkPos,
    chunk: &Chunk,
    other: Option<&Chunk>,
    lx: usize,
    lz: usize,
    (y0, y1): (i32, i32),
    scope: &Scope,
) {
    let mut orig_top = None;
    let mut recon_top = None;
    for y in (y0..=y1).rev() {
        let o = solid(chunk.block(lx, y, lz));
        let r = solid(other.and_then(|c| c.block(lx, y, lz)));
        rep.all.total += 1;
        if o.is_none() && r.is_none() {
            rep.all.exact += 1;
            rep.all.name += 1;
            continue;
        }
        let exact = o == r;
        let name = o.map(|b| &b.name) == r.map(|b| &b.name);
        rep.occupied.total += 1;
        for acc in [&mut rep.all, &mut rep.occupied] {
            acc.exact += exact as u64;
            acc.name += name as u64;
        }
        if orig_top.is_none() && o.is_some() {
            orig_top = Some((y, o));
        }
        if recon_top.is_none() && r.is_some() {
            recon_top = Some((y, r));
        }
        if o.is_some() {
            rep.solid.original += 1;
            let cell = (pos.0 * 16 + lx as i32, y, pos.1 * 16 + lz as i32);
            if scope.rendered.is_some_and(|r| r.contains(&cell)) {
                rep.rendered.total += 1;
                rep.rendered.exact += exact as u64;
                rep.rendered.name += name as u64;
                let key = (!exact).then(|| (label(o), label(r)));
                let alike = key.as_ref().is_none_or(|(a, b)| {
                    scope.lookalikes.is_some_and(|g| g.get(a).is_some_and(|ga| g.get(b) == Some(ga)))
                });
                rep.rendered_alike += alike as u64;
                if let Some(key) = key.filter(|_| !alike) {
                    *rep.rendered_confusion_counts.entry(key).or_default() += 1;
                }
            }
            if exposed(orig, pos, chunk, lx, y, lz, y0, y1) {
                rep.exposed.total += 1;
                rep.exposed.exact += exact as u64;
                rep.exposed.name += name as u64;
            }
        }
        if r.is_some() {
            rep.solid.reconstructed += 1;
        }
        if o.is_some() && r.is_some() {
            rep.solid.both += 1;
        }
        if !exact {
            let key = (label(o), label(r));
            if rep.samples.len() < MAX_SAMPLES && scope.sample.is_some_and(|(a, b)| a == key.0 && b == key.1) {
                rep.samples.push((pos.0 * 16 + lx as i32, y, pos.1 * 16 + lz as i32));
            }
            *rep.confusion_counts.entry(key).or_default() += 1;
        }
    }
    if orig_top.is_some() || recon_top.is_some() {
        rep.surface.total += 1;
        rep.surface.hits += (orig_top == recon_top) as u64;
    }
}

fn solid(b: Option<&BlockState>) -> Option<&BlockState> {
    b.filter(|b| !b.is_air())
}

fn label(b: Option<&BlockState>) -> String {
    b.map_or_else(|| "minecraft:air".to_owned(), ToString::to_string)
}

/// Any of the 6 neighbours is air. Below the world is void (air); a neighbour chunk that is not loaded
/// in this region counts as solid (conservative).
#[allow(clippy::too_many_arguments)]
fn exposed(orig: &Chunks, pos: ChunkPos, chunk: &Chunk, lx: usize, y: i32, lz: usize, y0: i32, y1: i32) -> bool {
    if y == y0 || y == y1 || air(chunk.block(lx, y - 1, lz)) || air(chunk.block(lx, y + 1, lz)) {
        return true;
    }
    [(-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().any(|(dx, dz)| {
        let (nx, nz) = (lx as i32 + dx, lz as i32 + dz);
        if (0..16).contains(&nx) && (0..16).contains(&nz) {
            return air(chunk.block(nx as usize, y, nz as usize));
        }
        let npos = (pos.0 + nx.div_euclid(16), pos.1 + nz.div_euclid(16));
        orig.get(&npos)
            .is_some_and(|c| air(c.block(nx.rem_euclid(16) as usize, y, nz.rem_euclid(16) as usize)))
    })
}

fn air(b: Option<&BlockState>) -> bool {
    b.is_none_or(BlockState::is_air)
}

fn score_biomes(rep: &mut Report, pos: ChunkPos, chunk: &Chunk, other: Option<&Chunk>, filter: ColumnFilter) {
    for s in &chunk.sections {
        let recon = other.and_then(|c| c.section(s.y));
        for cz in 0..4 {
            for cx in 0..4 {
                if !filter(pos.0 * 16 + cx as i32 * 4, pos.1 * 16 + cz as i32 * 4) {
                    continue;
                }
                for cy in 0..4 {
                    rep.biomes.total += 1;
                    let hit = recon.is_some_and(|r| r.biome(cx, cy, cz) == s.biome(cx, cy, cz));
                    rep.biomes.hits += hit as u64;
                }
            }
        }
    }
}
