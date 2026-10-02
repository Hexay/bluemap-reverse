//! Biome tints: the colour BlueMap multiplies grass, foliage and water textures by. Collected per 4×4
//! biome column cell while inverting, and learned per biome for packs (see bmr-fill/src/biomes.rs).

use anyhow::Result;
use bmr_fetch::LocalMap;
use bmr_world::World;
use rustc_hash::FxHashMap;

use crate::face::{Tex, WorldFace, texture_ids, world_faces};

pub const CHANNELS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Grass = 0,
    Foliage = 1,
    Water = 2,
}

/// Which biome colour a tinted texture shows (leaves with a fixed colour are untinted and never asked).
pub fn channel(tex: Tex) -> Option<Channel> {
    let name = tex.name();
    let n = name.trim_start_matches("minecraft:block/");
    if n.starts_with("water_") {
        Some(Channel::Water)
    } else if n.contains("leaves") || n == "vine" {
        Some(Channel::Foliage)
    } else if n.starts_with("grass_block") || n == "short_grass" || n.starts_with("tall_grass") || n.ends_with("fern") {
        Some(Channel::Grass)
    } else {
        None
    }
}

/// Mean tint per channel over some faces.
#[derive(Debug, Default, Clone)]
pub struct TintSum {
    sum: [[u64; 3]; CHANNELS],
    n: [u32; CHANNELS],
}

impl TintSum {
    pub fn add(&mut self, ch: Channel, color: [u8; 3]) {
        let c = ch as usize;
        (0..3).for_each(|a| self.sum[c][a] += color[a] as u64);
        self.n[c] += 1;
    }

    pub fn merge(&mut self, o: &TintSum) {
        for c in 0..CHANNELS {
            (0..3).for_each(|a| self.sum[c][a] += o.sum[c][a]);
            self.n[c] += o.n[c];
        }
    }

    pub fn means(&self) -> [Option<[u8; 3]>; CHANNELS] {
        std::array::from_fn(|c| (self.n[c] > 0).then(|| self.sum[c].map(|s| (s / self.n[c] as u64) as u8)))
    }
}

/// The tint BlueMap draws for one biome (learned from its render of the `biomes` fixture).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiomeTint {
    pub biome: String,
    /// grass, foliage, water
    pub tints: [Option<[u8; 3]>; CHANNELS],
}

/// Commonest overworld biomes first: they win ties between biomes that draw the same colours.
const COMMON: [&str; 16] = [
    "plains",
    "forest",
    "ocean",
    "deep_ocean",
    "river",
    "taiga",
    "birch_forest",
    "savanna",
    "desert",
    "beach",
    "dark_forest",
    "swamp",
    "jungle",
    "snowy_plains",
    "cold_ocean",
    "lukewarm_ocean",
];

/// Learn every biome's tints from BlueMap's render (`map`) of the `biomes` fixture (`world`): each tinted
/// face counts for the biome of its cell, unless another biome lies within 2 blocks (BlueMap blends
/// colours across biome borders).
pub fn learn(map: &LocalMap, world: &World) -> Result<Vec<BiomeTint>> {
    let names = texture_ids(&bmr_prbm::parse_texture_names(&map.textures_json()?)?);
    let mut chunks = FxHashMap::default();
    for r in world.regions()? {
        chunks.extend(world.read_region(r)?);
    }
    let biome = |(x, y, z): (i32, i32, i32)| -> Option<&str> {
        let c = chunks.get(&(x.div_euclid(16), z.div_euclid(16)))?;
        c.section(y.div_euclid(16))?.biome(
            (x.rem_euclid(16) / 4) as usize,
            (y.rem_euclid(16) / 4) as usize,
            (z.rem_euclid(16) / 4) as usize,
        )
    };
    let mut per_biome: FxHashMap<String, TintSum> = FxHashMap::default();
    for tile in map.tiles(0) {
        let faces = world_faces(&bmr_prbm::parse(&map.tile_bytes(0, tile)?)?, map.hires_origin(tile), &names);
        for f in faces.iter().filter(|f| f.color != [255, 255, 255]) {
            let Some(ch) = channel(f.texture) else { continue };
            let (x, y, z) = f.owner();
            let Some(b) = biome((x, y, z)) else { continue };
            let pure = (-2..=2).all(|dx| (-2..=2).all(|dz| biome((x + dx, y, z + dz)) == Some(b)));
            if pure {
                per_biome.entry(b.to_owned()).or_default().add(ch, f.color);
            }
        }
    }
    let rank = |b: &str| COMMON.iter().position(|c| b.trim_start_matches("minecraft:") == *c).unwrap_or(COMMON.len());
    let mut table: Vec<BiomeTint> =
        per_biome.into_iter().map(|(biome, s)| BiomeTint { tints: s.means(), biome }).collect();
    table.sort_by(|a, b| rank(&a.biome).cmp(&rank(&b.biome)).then_with(|| a.biome.cmp(&b.biome)));
    Ok(table)
}

/// Add each tinted grass/foliage/water face to its 4×4 biome column cell.
pub fn collect(faces: &[WorldFace], out: &mut FxHashMap<(i32, i32), TintSum>) {
    for f in faces.iter().filter(|f| f.color != [255, 255, 255]) {
        if let Some(ch) = channel(f.texture) {
            let (x, _, z) = f.owner();
            out.entry((x >> 2, z >> 2)).or_default().add(ch, f.color);
        }
    }
}

/// Biomes by distance of their tints to the observed ones (mean squared RGB error over the channels both
/// have), nearest first; equal distances keep `table` order (packs order it commonest first).
pub fn ranked<'a>(table: &'a [BiomeTint], seen: &[Option<[u8; 3]>; CHANNELS]) -> Vec<(u32, &'a str)> {
    let mut out: Vec<(u32, &str)> = table
        .iter()
        .filter_map(|b| {
            let shared: Vec<u32> = (0..CHANNELS)
                .filter_map(|c| Some((b.tints[c]?, seen[c]?)))
                .map(|(a, o)| (0..3).map(|i| (a[i] as i32 - o[i] as i32).pow(2) as u32).sum())
                .collect();
            (!shared.is_empty()).then(|| (shared.iter().sum::<u32>() / shared.len() as u32, b.biome.as_str()))
        })
        .collect();
    out.sort_by_key(|&(d, _)| d);
    out
}
