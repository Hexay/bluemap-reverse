//! Which of a site's maps to reconstruct, and into which dimension of the one output world.

use std::path::Path;

use anyhow::{Result, bail};
use bmr_fetch::LocalMap;
use bmr_world::Dimension;

pub struct Planned {
    pub id: String,
    pub map: LocalMap,
    pub dimension: Dimension,
}

/// Every mirrored map (or only `only`), overworld first. Two maps of one dimension are separate worlds
/// and cannot share the output: the caller must pick one.
pub fn choose(cache: &Path, ids: &[String], only: Option<&str>, dimension: Option<&Dimension>) -> Result<Vec<Planned>> {
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
        let dimension = dimension.map_or_else(|| guess_dimension(id, map.settings.sky_color), Clone::clone);
        if let Some(other) = out.iter().find(|p: &&Planned| p.dimension == dimension) {
            bail!("maps `{}` and `{id}` are both {dimension}, i.e. separate worlds: pick one with --map", other.id);
        }
        out.push(Planned { id: id.to_owned(), map, dimension });
    }
    out.sort_by_key(|p| ORDER.iter().position(|d| *d == p.dimension));
    Ok(out)
}

const ORDER: [Dimension; 3] = [Dimension::Overworld, Dimension::Nether, Dimension::End];

/// Dimension of a map from its id (`world_nether`, `world_the_end`, `nether`…), else from BlueMap's
/// default sky colour for the dimension (nether #290000, end #080010), else the overworld.
pub fn guess_dimension(map_id: &str, sky: Option<[f32; 4]>) -> Dimension {
    let id = map_id.to_ascii_lowercase();
    let near = |c: [f32; 4], hex: [u8; 3]| (0..3).all(|i| (c[i] - hex[i] as f32 / 255.0).abs() < 0.01);
    if id.contains("nether") || sky.is_some_and(|c| near(c, [0x29, 0, 0])) {
        Dimension::Nether
    } else if id == "end" || id.ends_with("_end") || sky.is_some_and(|c| near(c, [0x08, 0, 0x10])) {
        Dimension::End
    } else {
        Dimension::Overworld
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_from_id_or_sky() {
        assert_eq!(guess_dimension("world_nether", None), Dimension::Nether);
        assert_eq!(guess_dimension("world_the_end", None), Dimension::End);
        assert_eq!(guess_dimension("survival", Some([0x29 as f32 / 255.0, 0.0, 0.0, 1.0])), Dimension::Nether);
        assert_eq!(guess_dimension("world", Some([0.49, 0.67, 1.0, 1.0])), Dimension::Overworld);
    }
}
