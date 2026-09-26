//! Biomes without a seed. Minecraft stores one biome per 4×4×4 cell. The nether's are told by what grows on
//! them (nylium, stems, soul sand, basalt), voted over each cell and its neighbours; the end's by distance
//! from the main island. Overworld biomes need tints and stay at the chunk default.

use rustc_hash::FxHashMap;

use bmr_invert::face::Cell;
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

pub struct Biomes {
    kind: Kind,
    /// Nether: winning biome per 4×4×4 cell with evidence nearby.
    decided: FxHashMap<Cell, &'static str>,
}

impl Biomes {
    pub fn from_blocks(blocks: &FxHashMap<Cell, BlockState>, profile: &Profile) -> Self {
        let mut decided = FxHashMap::default();
        if profile.kind == Kind::Nether {
            let mut votes: FxHashMap<Cell, [f32; 5]> = FxHashMap::default();
            for (&(x, y, z), s) in blocks {
                let name = s.name.trim_start_matches("minecraft:");
                if let Some(&(_, biome, w)) = MARKERS.iter().find(|(m, _, _)| name.contains(m)) {
                    votes.entry((x >> 2, y >> 2, z >> 2)).or_default()[biome] += w;
                }
            }
            for &(cx, cy, cz) in votes.keys() {
                for n in neighbours((cx, cy, cz)) {
                    decided.entry(n).or_insert_with(|| {
                        let mut sum = [0f32; 5];
                        for m in neighbours(n) {
                            if let Some(v) = votes.get(&m) {
                                (0..5).for_each(|i| sum[i] += v[i]);
                            }
                        }
                        NETHER[(0..5).max_by(|&a, &b| sum[a].total_cmp(&sum[b])).unwrap_or(0)]
                    });
                }
            }
        }
        Self { kind: profile.kind, decided }
    }

    /// Biome of the 4×4×4 cell at cell coordinates `c`; `None` keeps the chunk's default.
    pub fn at(&self, c @ (cx, cy, cz): Cell) -> Option<&'static str> {
        match self.kind {
            Kind::Overworld => None,
            Kind::End => {
                let (x, z) = (cx as i64 * 4, cz as i64 * 4);
                Some(if x * x + z * z <= END_ISLAND * END_ISLAND { "minecraft:the_end" } else { "minecraft:end_highlands" })
            }
            // no evidence within a cell: the nearest decided cell straight up or down, else the commonest biome
            Kind::Nether => Some(
                self.decided
                    .get(&c)
                    .or_else(|| (1..16).find_map(|d| self.decided.get(&(cx, cy - d, cz)).or(self.decided.get(&(cx, cy + d, cz)))))
                    .copied()
                    .unwrap_or(NETHER[0]),
            ),
        }
    }
}

fn neighbours((x, y, z): Cell) -> impl Iterator<Item = Cell> {
    (-1..=1).flat_map(move |dx| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| (x + dx, y + dy, z + dz))))
}
