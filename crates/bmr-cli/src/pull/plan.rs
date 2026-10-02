//! Which of a site's maps to reconstruct, and into which dimension of the one output world.

use std::path::Path;

use anyhow::{Result, bail};
use bmr_fetch::LocalMap;

pub struct Planned {
    pub id: String,
    pub map: LocalMap,
    pub dimension: String,
}

/// Every mirrored map (or only `only`), overworld first. Two maps of one dimension are separate worlds
/// and cannot share the output: the caller must pick one.
pub fn choose(cache: &Path, ids: &[String], only: Option<&str>, dimension: Option<&str>) -> Result<Vec<Planned>> {
    let ids: Vec<&str> = match only {
        Some(m) => vec![m],
        None => ids.iter().map(String::as_str).collect(),
    };
    if dimension.is_some() && ids.len() > 1 {
        bail!("--dimension applies to one map: pick it with --map (site maps: {ids:?})");
    }
    let mut out = Vec::new();
    for id in ids {
        let map = LocalMap::open(cache, Some(id))?;
        let dimension = dimension.map_or_else(|| guess_dimension(id, map.settings.sky_color), str::to_owned);
        if let Some(other) = out.iter().find(|p: &&Planned| p.dimension == dimension) {
            bail!("maps `{}` and `{id}` are both {dimension}, i.e. separate worlds: pick one with --map", other.id);
        }
        out.push(Planned { id: id.to_owned(), map, dimension });
    }
    out.sort_by_key(|p| ORDER.iter().position(|d| *d == p.dimension));
    Ok(out)
}

const ORDER: [&str; 3] = ["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end"];

/// Dimension of a map from its id (`world_nether`, `world_the_end`, `nether`…), else from BlueMap's
/// default sky colour for the dimension (nether #290000, end #080010), else the overworld.
pub fn guess_dimension(map_id: &str, sky: Option<[f32; 4]>) -> String {
    let id = map_id.to_ascii_lowercase();
    let near = |c: [f32; 4], hex: [u8; 3]| (0..3).all(|i| (c[i] - hex[i] as f32 / 255.0).abs() < 0.01);
    let dim = if id.contains("nether") || sky.is_some_and(|c| near(c, [0x29, 0, 0])) {
        ORDER[1]
    } else if id == "end" || id.ends_with("_end") || sky.is_some_and(|c| near(c, [0x08, 0, 0x10])) {
        ORDER[2]
    } else {
        ORDER[0]
    };
    dim.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_from_id_or_sky() {
        assert_eq!(guess_dimension("world_nether", None), "minecraft:the_nether");
        assert_eq!(guess_dimension("world_the_end", None), "minecraft:the_end");
        assert_eq!(guess_dimension("survival", Some([0x29 as f32 / 255.0, 0.0, 0.0, 1.0])), "minecraft:the_nether");
        assert_eq!(guess_dimension("world", Some([0.49, 0.67, 1.0, 1.0])), "minecraft:overworld");
    }
}
