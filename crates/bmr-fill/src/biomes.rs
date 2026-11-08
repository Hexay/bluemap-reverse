//! Biomes without a seed. Minecraft stores one biome per 4×4×4 cell. Overworld biomes are told by the grass,
//! foliage and water tints BlueMap drew (nearest entry of the pack's learned table, per 4×4 column); the
//! nether's by what grows there (nylium, stems, soul sand, basalt), voted over each 4×4 column and its
//! neighbours; the end's by distance from the main island.

use std::collections::VecDeque;
use std::collections::hash_map::Entry;

use rustc_hash::{FxHashMap, FxHashSet};

use bmr_invert::face::Cell;
use bmr_invert::tints::{BiomeTint, TintSum, ranked};
use bmr_world::BlockState;

use crate::profile::{Kind, Profile};

const NETHER: [&str; 5] = [
    "minecraft:nether_wastes",
    "minecraft:soul_sand_valley",
    "minecraft:crimson_forest",
    "minecraft:warped_forest",
    "minecraft:basalt_deltas",
];

/// (block name contains, biome index into NETHER, vote weight). Netherrack underlies every nether biome,
/// so it only decides where nothing else is seen.
const MARKERS: [(&str, usize, f32); 14] = [
    ("soul_sand", 1, 1.0),
    ("soul_soil", 1, 1.0),
    ("crimson", 2, 1.0),
    ("nether_wart_block", 2, 1.0),
    ("weeping_vines", 2, 1.0),
    ("warped", 3, 1.0),
    ("nether_sprouts", 3, 1.0),
    ("twisting_vines", 3, 1.0),
    ("basalt", 4, 1.0),
    ("blackstone", 4, 1.0),
    ("netherrack", 0, 0.2),
    ("nether_quartz_ore", 0, 0.2),
    ("nether_gold_ore", 0, 0.2),
    ("glowstone", 0, 0.2),
];

/// Radius of the main end island's biome (vanilla: `the_end` within 1024 blocks of the origin).
const END_ISLAND: i64 = 1024;
/// How far (in 4-block columns) an overworld column without tints looks for a tinted one.
const SPREAD: i32 = 8;

/// How far (in 4-block columns) a nether column without markers looks for a decided one.
const NETHER_SPREAD: i32 = 64;

pub struct Biomes {
    kind: Kind,
    /// Overworld and nether: biome per 4×4 column, spread from the ones with evidence.
    columns: FxHashMap<(i32, i32), String>,
}

impl Biomes {
    /// `tints`: what was seen per 4×4 column; `table`: the pack's biome tints (empty: overworld unknown).
    pub fn new(
        blocks: &FxHashMap<Cell, BlockState>,
        profile: &Profile,
        tints: &FxHashMap<(i32, i32), TintSum>,
        table: &[BiomeTint],
    ) -> Self {
        let columns = match profile.kind {
            Kind::Nether => nether_columns(blocks),
            Kind::Overworld if !table.is_empty() => overworld_columns(blocks, tints, table),
            _ => FxHashMap::default(),
        };
        Self { kind: profile.kind, columns }
    }

    /// Biome of the 4×4×4 cell at cell coordinates `c`; `None` keeps the chunk's default.
    pub fn at(&self, (cx, _, cz): Cell) -> Option<&str> {
        match self.kind {
            Kind::Overworld => self.columns.get(&(cx, cz)).map(String::as_str),
            Kind::End => {
                let (x, z) = (cx as i64 * 4, cz as i64 * 4);
                Some(if x * x + z * z <= END_ISLAND * END_ISLAND {
                    "minecraft:the_end"
                } else {
                    "minecraft:end_highlands"
                })
            }
            Kind::Nether => Some(self.columns.get(&(cx, cz)).map_or(NETHER[0], String::as_str)),
        }
    }
}

/// Nether biomes don't vary with height (the climate noise is 2D), so a column's markers vote at every depth:
/// open caves and the floor far below decide the cells in between. Columns with no markers take the
/// nearest decided column's biome.
fn nether_columns(blocks: &FxHashMap<Cell, BlockState>) -> FxHashMap<(i32, i32), String> {
    let mut votes: FxHashMap<(i32, i32), [f32; 5]> = FxHashMap::default();
    for (&(x, _, z), s) in blocks {
        let name = s.name.trim_start_matches("minecraft:");
        if let Some(&(_, biome, w)) = MARKERS.iter().find(|(m, _, _)| name.contains(m)) {
            votes.entry((x >> 2, z >> 2)).or_default()[biome] += w;
        }
    }
    let mut out = FxHashMap::default();
    for &(x, z) in votes.keys() {
        let mut sum = [0f32; 5];
        let votes = &votes;
        for v in (-1..=1).flat_map(|dx| (-1..=1).filter_map(move |dz| votes.get(&(x + dx, z + dz)))) {
            (0..5).for_each(|i| sum[i] += v[i]);
        }
        let best = (0..5).max_by(|&a, &b| sum[a].total_cmp(&sum[b])).unwrap_or(0);
        out.insert((x, z), NETHER[best].to_owned());
    }
    spread(&mut out, NETHER_SPREAD);
    out
}

/// Tinted columns take the nearest biome of `table`, ties (beach = plains colours) settled by the surface;
/// the rest take the nearest tinted column's biome, up to SPREAD away.
fn overworld_columns(
    blocks: &FxHashMap<Cell, BlockState>,
    tints: &FxHashMap<(i32, i32), TintSum>,
    table: &[BiomeTint],
) -> FxHashMap<(i32, i32), String> {
    let sandy = sandy_columns(blocks);
    let mut out: FxHashMap<(i32, i32), String> = FxHashMap::default();
    for (&col, t) in tints {
        let ranked = ranked(table, &t.means());
        let Some(&(best, first)) = ranked.first() else { continue };
        let tied = ranked.iter().take_while(|(d, _)| *d <= best + TIE).map(|&(_, b)| b);
        // mostly sand on top: a beach, if one draws these colours (water depth does not tell ocean from
        // deep ocean or river from beach: measured on the vanilla fixture, all overlap)
        let b = if sandy.contains(&col) { tied.clone().find(|b| b.ends_with("beach")).unwrap_or(first) } else { first };
        out.insert(col, b.to_owned());
    }
    spread(&mut out, SPREAD);
    out
}

/// Breadth-first: every undecided column within `max` steps of a decided one takes the nearest one's biome.
fn spread(out: &mut FxHashMap<(i32, i32), String>, max: i32) {
    let mut queue: VecDeque<((i32, i32), i32)> = out.keys().map(|&c| (c, 0)).collect();
    while let Some(((x, z), d)) = queue.pop_front() {
        if d == max {
            continue;
        }
        let b = out[&(x, z)].clone();
        for n in [(x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)] {
            if let Entry::Vacant(e) = out.entry(n) {
                e.insert(b.clone());
                queue.push_back((n, d + 1));
            }
        }
    }
}

/// Tint distance (mean squared RGB error) within which biomes count as drawing the same colours.
const TIE: u32 = 20;

/// 4×4 columns whose top blocks are mostly sand or sandstone (water not counted).
fn sandy_columns(blocks: &FxHashMap<Cell, BlockState>) -> FxHashSet<(i32, i32)> {
    let mut top: FxHashMap<(i32, i32), (i32, &str)> = FxHashMap::default();
    for (&(x, y, z), s) in blocks {
        if top.get(&(x, z)).is_none_or(|&(ty, _)| y > ty) {
            top.insert((x, z), (y, s.name.as_str()));
        }
    }
    let mut count: FxHashMap<(i32, i32), (u32, u32)> = FxHashMap::default();
    for (&(x, z), &(_, name)) in &top {
        let (sand, land) = count.entry((x >> 2, z >> 2)).or_default();
        match name {
            "minecraft:sand" | "minecraft:sandstone" => *sand += 1,
            "minecraft:water" => {}
            _ => *land += 1,
        }
    }
    count.into_iter().filter(|(_, (sand, land))| sand > land).map(|(c, _)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nether_biome_holds_for_the_whole_column_and_spreads() {
        let soul = BlockState::new("minecraft:soul_sand".into(), vec![]);
        let blocks: FxHashMap<Cell, BlockState> = (0..4).map(|x| ((x, 30, 0), soul.clone())).collect();
        let b = Biomes::new(&blocks, &Profile::for_dimension("minecraft:the_nether"), &FxHashMap::default(), &[]);
        let valley = Some("minecraft:soul_sand_valley");
        assert_eq!(b.at((0, 7, 0)), valley, "marker cell");
        assert_eq!(b.at((0, 25, 0)), valley, "far above the marker, same column");
        assert_eq!(b.at((5, 2, 3)), valley, "column without markers takes the nearest decided one");
        let empty = Biomes::new(
            &FxHashMap::default(),
            &Profile::for_dimension("minecraft:the_nether"),
            &FxHashMap::default(),
            &[],
        );
        assert_eq!(empty.at((0, 0, 0)), Some(NETHER[0]));
    }
}
