//! Which chunks a map rendered, and the `forceload` rectangles that generate exactly those.

use std::collections::{BTreeMap, BTreeSet};

use bmr_fetch::{BlockBox, LocalMap};

/// `forceload add` refuses a larger area.
const FORCELOAD_MAX_CHUNKS: i32 = 256;
/// Chunks held loaded at once; each batch is released before the next so the server's memory stays flat.
const BATCH_MAX_CHUNKS: i32 = 1024;

/// (chunk x, chunk z)
pub type Chunk = (i32, i32);

/// Inclusive rectangle of chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkRect {
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
}

impl ChunkRect {
    fn area(&self) -> i32 {
        (self.x1 - self.x0 + 1) * (self.z1 - self.z0 + 1)
    }

    /// `action` is `add` or `remove`.
    pub fn forceload(&self, dimension: &str, action: &str) -> String {
        let (x0, z0, x1, z1) = (self.x0 * 16, self.z0 * 16, self.x1 * 16 + 15, self.z1 * 16 + 15);
        format!("execute in {dimension} run forceload {action} {x0} {z0} {x1} {z1}")
    }

    /// Block x/z of the four corners: the last chunks of a rectangle to load are among these.
    pub fn corners(&self) -> [(i32, i32); 4] {
        let (x0, z0, x1, z1) = (self.x0 * 16, self.z0 * 16, self.x1 * 16, self.z1 * 16);
        [(x0, z0), (x0, z1), (x1, z0), (x1, z1)]
    }
}

/// Chunks touched by the map's hires tiles (clipped to `area`).
pub fn rendered_chunks(map: &LocalMap, area: Option<BlockBox>) -> BTreeSet<Chunk> {
    let size = map.settings.hires_grid().size;
    let mut out = BTreeSet::new();
    for t in map.tiles(0) {
        out.extend(tile_chunks(map.hires_origin(t), size, area));
    }
    out
}

fn tile_chunks([x, z]: [i32; 2], [w, h]: [i32; 2], area: Option<BlockBox>) -> Vec<Chunk> {
    let tile = BlockBox::new(x, z, x + w - 1, z + h - 1);
    let Some(b) = area.map_or(Some(tile), |a| a.intersect(tile)) else { return Vec::new() };
    (b.min[1] >> 4..=b.max[1] >> 4).flat_map(|cz| (b.min[0] >> 4..=b.max[0] >> 4).map(move |cx| (cx, cz))).collect()
}

/// Cover `chunks` exactly with rectangles of at most [`FORCELOAD_MAX_CHUNKS`]: runs per row, stacked while
/// the next row has the same run.
pub fn rectangles(chunks: &BTreeSet<Chunk>) -> Vec<ChunkRect> {
    let mut by_row: Vec<(i32, i32)> = chunks.iter().map(|&(x, z)| (z, x)).collect();
    by_row.sort_unstable();
    let mut rects: Vec<ChunkRect> = Vec::new();
    let mut open: BTreeMap<(i32, i32), usize> = BTreeMap::new();
    let mut place = |z: i32, x0: i32, x1: i32| {
        let width = x1 - x0 + 1;
        if let Some(&i) = open.get(&(x0, x1))
            && rects[i].z1 == z - 1
            && rects[i].area() + width <= FORCELOAD_MAX_CHUNKS
        {
            rects[i].z1 = z;
            return;
        }
        open.insert((x0, x1), rects.len());
        rects.push(ChunkRect { x0, z0: z, x1, z1: z });
    };
    let mut i = 0;
    while i < by_row.len() {
        let (z, x0) = by_row[i];
        let mut end = i;
        while by_row.get(end + 1) == Some(&(z, by_row[end].1 + 1)) && by_row[end + 1].1 - x0 < FORCELOAD_MAX_CHUNKS {
            end += 1;
        }
        place(z, x0, by_row[end].1);
        i = end + 1;
    }
    rects
}

/// Consecutive groups of rectangles of at most [`BATCH_MAX_CHUNKS`] chunks.
pub fn batches(rects: &[ChunkRect]) -> Vec<&[ChunkRect]> {
    let mut out = Vec::new();
    let (mut start, mut area) = (0, 0);
    for (i, r) in rects.iter().enumerate() {
        if area + r.area() > BATCH_MAX_CHUNKS {
            out.push(&rects[start..i]);
            (start, area) = (i, 0);
        }
        area += r.area();
    }
    if start < rects.len() {
        out.push(&rects[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(chunks: &[Chunk]) -> BTreeSet<Chunk> {
        chunks.iter().copied().collect()
    }

    fn covered(rects: &[ChunkRect]) -> BTreeSet<Chunk> {
        rects.iter().flat_map(|r| (r.z0..=r.z1).flat_map(move |z| (r.x0..=r.x1).map(move |x| (x, z)))).collect()
    }

    #[test]
    fn stacks_equal_runs_and_splits_different_ones() {
        // rows 0 and 1: x 0..=1; row 2: x 0..=2; a separate island at x 5
        let chunks = set(&[(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2), (2, 2), (5, 0)]);
        let rects = rectangles(&chunks);
        assert_eq!(covered(&rects), chunks);
        assert_eq!(rects.len(), 3);
    }

    #[test]
    fn respects_the_forceload_limit() {
        let chunks: BTreeSet<Chunk> = (0..600).flat_map(|x| (0..3).map(move |z| (x, z))).collect();
        let rects = rectangles(&chunks);
        assert_eq!(covered(&rects), chunks);
        assert!(rects.iter().all(|r| r.area() <= FORCELOAD_MAX_CHUNKS));
    }

    #[test]
    fn batches_cover_all_rectangles_within_the_cap() {
        let chunks: BTreeSet<Chunk> = (0..100).flat_map(|x| (0..100).map(move |z| (x, z))).collect();
        let rects = rectangles(&chunks);
        let groups = batches(&rects);
        assert_eq!(groups.iter().map(|g| g.len()).sum::<usize>(), rects.len());
        assert!(groups.iter().all(|g| g.iter().map(ChunkRect::area).sum::<i32>() <= BATCH_MAX_CHUNKS));
    }

    #[test]
    fn commands_use_block_coordinates() {
        let r = ChunkRect { x0: -1, z0: 0, x1: 0, z1: 1 };
        assert_eq!(
            r.forceload("minecraft:overworld", "add"),
            "execute in minecraft:overworld run forceload add -16 0 15 31"
        );
        assert_eq!(r.corners()[0], (-16, 0));
    }

    #[test]
    fn tile_chunks_follow_the_grid_offset_and_area() {
        // a 32-block tile at offset 2 straddles three chunks per axis
        assert_eq!(tile_chunks([2, 2], [32, 32], None).len(), 9);
        let area = BlockBox::new(0, 0, 15, 15);
        assert_eq!(tile_chunks([2, 2], [32, 32], Some(area)), vec![(0, 0)]);
        assert!(tile_chunks([100, 100], [32, 32], Some(area)).is_empty());
    }
}
