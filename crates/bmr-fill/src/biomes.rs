//! Biomes without a seed. Minecraft stores one biome per 4×4×4 cell. Overworld biomes are told by the grass,
//! foliage and water tints BlueMap drew (nearest entry of the pack's learned table, per 4×4 column); the
//! nether's by what grows there (nylium, stems, soul sand, basalt), voted over each cell and its neighbours;
//! the end's by distance from the main island.

use std::collections::VecDeque;
use std::collections::hash_map::Entry;

use rustc_hash::{FxHashMap, FxHashSet};

use bmr_invert::face::Cell;
use bmr_invert::tints::{BiomeTint, TintSum, ranked};
use bmr_world::BlockState;

use crate::profile::{Kind, Profile};

const NETHER: [&str; 5] =
    ["minecraft:nether_wastes", "minecraft:soul_sand_valley", "minecraft:crimson_forest", "minecraft:warped_forest", "minecraft:basalt_deltas"];

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

pub struct Biomes {
    kind: Kind,
    /// Nether: winning biome per 4×4×4 cell with evidence nearby.
    cells: FxHashMap<Cell, &'static str>,
    /// Overworld: biome per 4×4 column, spread from the tinted ones.
    columns: FxHashMap<(i32, i32), String>,
}

impl Biomes {
    /// `tints`: what was seen per 4×4 column; `table`: the pack's biome tints (empty: overworld unknown).
    pub fn new(blocks: &FxHashMap<Cell, BlockState>, profile: &Profile, tints: &FxHashMap<(i32, i32), TintSum>, table: &[BiomeTint]) -> Self {
        let mut me = Self { kind: profile.kind, cells: FxHashMap::default(), columns: FxHashMap::default() };
        match profile.kind {
            Kind::Nether => me.cells = nether_cells(blocks),
            Kind::Overworld if !table.is_empty() => me.columns = overworld_columns(blocks, tints, table),
            _ => {}
        }
        me
    }

    /// Biome of the 4×4×4 cell at cell coordinates `c`; `None` keeps the chunk's default.
    pub fn at(&self, c @ (cx, cy, cz): Cell) -> Option<&str> {
        match self.kind {
            Kind::Overworld => self.columns.get(&(cx, cz)).map(String::as_str),
            Kind::End => {
                let (x, z) = (cx as i64 * 4, cz as i64 * 4);
                Some(if x * x + z * z <= END_ISLAND * END_ISLAND { "minecraft:the_end" } else { "minecraft:end_highlands" })
            }
            // no evidence within a cell: the nearest decided cell straight up or down, else the commonest biome
            Kind::Nether => Some(
                self.cells
                    .get(&c)
                    .or_else(|| (1..16).find_map(|d| self.cells.get(&(cx, cy - d, cz)).or(self.cells.get(&(cx, cy + d, cz)))))
                    .copied()
                    .unwrap_or(NETHER[0]),
            ),
        }
    }
}

fn nether_cells(blocks: &FxHashMap<Cell, BlockState>) -> FxHashMap<Cell, &'static str> {
    let mut votes: FxHashMap<Cell, [f32; 5]> = FxHashMap::default();
    for (&(x, y, z), s) in blocks {
        let name = s.name.trim_start_matches("minecraft:");
        if let Some(&(_, biome, w)) = MARKERS.iter().find(|(m, _, _)| name.contains(m)) {
            votes.entry((x >> 2, y >> 2, z >> 2)).or_default()[biome] += w;
        }
    }
    let mut decided = FxHashMap::default();
    for &c in votes.keys() {
        for n in neighbours(c) {
            decided.entry(n).or_insert_with(|| {
                let mut sum = [0f32; 5];
                for v in neighbours(n).filter_map(|m| votes.get(&m)) {
                    (0..5).for_each(|i| sum[i] += v[i]);
                }
                NETHER[(0..5).max_by(|&a, &b| sum[a].total_cmp(&sum[b])).unwrap_or(0)]
            });
        }
    }
    decided
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
    let mut queue = VecDeque::new();
    for (&col, t) in tints {
        let ranked = ranked(table, &t.means());
        let Some(&(best, first)) = ranked.first() else { continue };
        let tied = ranked.iter().take_while(|(d, _)| *d <= best + TIE).map(|&(_, b)| b);
        // mostly sand on top: a beach, if one draws these colours (water depth does not tell ocean from
        // deep ocean or river from beach: measured on the vanilla fixture, all overlap)
        let b = if sandy.contains(&col) { tied.clone().find(|b| b.ends_with("beach")).unwrap_or(first) } else { first };
        out.insert(col, b.to_owned());
        queue.push_back((col, 0));
    }
    while let Some(((x, z), d)) = queue.pop_front() {
        if d == SPREAD {
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
    out
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

fn neighbours((x, y, z): Cell) -> impl Iterator<Item = Cell> {
    (-1..=1).flat_map(move |dx| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| (x + dx, y + dy, z + dz))))
}
