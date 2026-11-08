//! Structure templates (templates.json, from tools/gen_structure_templates.py) placed by an affine XZ transform and
//! scored against a world: the fraction of a template's signature blocks found at their exact positions.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use super::blocks::BlockSource;

pub struct Template {
    pub name: String,
    /// local positions and names of the signature blocks (terrain, light sources and doors left out)
    pub blocks: Vec<([i32; 3], String)>,
}

impl Template {
    pub fn new(name: &str, blocks: impl IntoIterator<Item = ([i32; 3], String)>) -> Self {
        let blocks = blocks.into_iter().filter(|(_, n)| !unreliable(n)).map(|(p, n)| (p, canonical(&n))).collect();
        Self { name: name.to_string(), blocks }
    }

    /// Local positions of `block`.
    pub fn find<'a>(&'a self, block: &'a str) -> impl Iterator<Item = [i32; 3]> + 'a {
        self.blocks.iter().filter(move |(_, n)| n == block).map(|(p, _)| *p)
    }
}

/// Blocks that terrain, processors (zombie villages drop torches and doors) or the map render don't reproduce.
fn unreliable(name: &str) -> bool {
    matches!(
        name,
        "cave_air"
            | "grass_block"
            | "dirt"
            | "short_grass"
            | "tall_grass"
            | "dandelion"
            | "sand"
            | "snow"
            | "water"
            | "torch"
            | "wall_torch"
            | "redstone_torch"
    ) || name.ends_with("_door")
}

/// Processors turn cobblestone mossy at random.
fn canonical(name: &str) -> String {
    name.strip_prefix("mossy_").filter(|n| n.starts_with("cobblestone")).unwrap_or(name).to_string()
}

#[derive(Deserialize)]
struct Raw {
    templates: HashMap<String, RawTemplate>,
}

#[derive(Deserialize)]
struct RawTemplate {
    blocks: HashMap<String, Vec<i32>>,
}

/// Every template in templates.json, by name (`village/plains/town_centers/plains_fountain_01`, `igloo/top`, …).
pub fn templates() -> &'static [Template] {
    static ALL: OnceLock<Vec<Template>> = OnceLock::new();
    ALL.get_or_init(|| {
        let raw: Raw = serde_json::from_str(include_str!("templates.json")).expect("templates.json");
        let mut out: Vec<Template> = raw
            .templates
            .iter()
            .map(|(name, t)| {
                let blocks =
                    t.blocks.iter().flat_map(|(n, xyz)| xyz.as_chunks::<3>().0.iter().map(|p| (*p, n.clone())));
                Template::new(name, blocks)
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    })
}

pub fn template(name: &str) -> &'static Template {
    templates().iter().find(|t| t.name == name).unwrap_or_else(|| panic!("template {name} missing"))
}

/// Pivot-0 rotations, `StructureTemplate.transform` order: NONE, CW90, CW180, CCW90.
pub const ROTATIONS: [[i32; 4]; 4] = [[1, 0, 0, 1], [0, -1, 1, 0], [-1, 0, 0, -1], [0, 1, -1, 0]];

/// world = t + (m · local.xz, local.y); `m` row-major 2×2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transform {
    pub m: [i32; 4],
    pub t: [i32; 3],
}

impl Transform {
    /// Template placed at `pos`, rotated by `m` about `pivot` (local xz).
    pub fn rotated(m: [i32; 4], pivot: [i32; 2], pos: [i32; 3]) -> Self {
        let [px, pz] = pivot;
        let t = [pos[0] + px - (m[0] * px + m[1] * pz), pos[1], pos[2] + pz - (m[2] * px + m[3] * pz)];
        Self { m, t }
    }

    pub fn apply(&self, [x, y, z]: [i32; 3]) -> [i32; 3] {
        [self.t[0] + self.m[0] * x + self.m[1] * z, self.t[1] + y, self.t[2] + self.m[2] * x + self.m[3] * z]
    }
}

/// One way a structure could have produced the observed blocks: a template, where it went, and the start chunk
/// that placement implies.
pub struct Hypothesis {
    pub template: &'static Template,
    pub at: Transform,
    pub start: [i32; 2],
}

impl Hypothesis {
    /// World XZ chunks the placed template touches.
    pub fn chunks(&self) -> impl Iterator<Item = [i32; 2]> + '_ {
        self.template.blocks.iter().map(|(p, _)| {
            let w = self.at.apply(*p);
            [w[0].div_euclid(16), w[2].div_euclid(16)]
        })
    }

    /// Fraction of the template's signature blocks present in `world`.
    pub fn score(&self, world: &dyn BlockSource) -> f64 {
        let hits = self
            .template
            .blocks
            .iter()
            .filter(|(p, n)| world.name_at(self.at.apply(*p)).is_some_and(|w| canonical(w) == *n))
            .count();
        hits as f64 / self.template.blocks.len().max(1) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_about_a_pivot_matches_the_research_table() {
        // igloo/top, pivot (3,5): CW90 maps local (0,0) → (8,2) and (6,7) → (1,8) (structure_start_rules.md §2)
        let cw90 = Transform::rotated(ROTATIONS[1], [3, 5], [0, 0, 0]);
        assert_eq!(cw90.apply([0, 0, 0]), [8, 0, 2]);
        assert_eq!(cw90.apply([6, 0, 7]), [1, 0, 8]);
    }

    #[test]
    fn templates_load_without_terrain() {
        let fountain = template("village/plains/town_centers/plains_fountain_01");
        assert_eq!(fountain.find("bell").map(|p| (p[0], p[2])).collect::<Vec<_>>(), vec![(4, 2)]);
        assert!(fountain.blocks.iter().all(|(_, n)| n != "water" && n != "torch"));
        assert!(templates().len() >= 34);
    }
}
