//! A flat world's own layering by height, learned from evidence (observed terrain surfaces and the layers
//! implied under them): a superflat's bedrock/dirt/grass instead of the overworld's bedrock band and
//! deepslate. Only for maps whose terrain surface mostly sits at one height: per-height agreement alone
//! also holds on natural seabeds (sand at y≈45 over the whole vanilla fixture), where it filled stone with
//! sand (vanilla occupied 75.3% → 74.1%).

use rustc_hash::FxHashMap;

/// Share of samples at a height that must agree.
const SHARE: f32 = 0.9;
/// Samples needed at a height before their majority counts.
const MIN_SAMPLES: u32 = 64;
/// Share of terrain surfaces at the commonest surface height for the map to count as flat.
const FLAT_SHARE: f32 = 0.5;

#[derive(Default)]
pub struct Typical {
    by_y: FxHashMap<i32, String>,
}

impl Typical {
    /// `surfaces`: heights of observed terrain surfaces; `samples`: (y, block name) evidence, one per column
    /// and height. Empty unless the surfaces are flat.
    pub fn learn<'a>(surfaces: &[i32], samples: impl IntoIterator<Item = (i32, &'a str)>) -> Self {
        let mut at_height: FxHashMap<i32, u32> = FxHashMap::default();
        surfaces.iter().for_each(|&y| *at_height.entry(y).or_default() += 1);
        let modal = at_height.values().max().copied().unwrap_or(0);
        if surfaces.len() < MIN_SAMPLES as usize || (modal as f32) < FLAT_SHARE * surfaces.len() as f32 {
            return Self::default();
        }
        let mut counts: FxHashMap<i32, FxHashMap<&str, u32>> = FxHashMap::default();
        for (y, name) in samples {
            *counts.entry(y).or_default().entry(name).or_default() += 1;
        }
        let by_y = counts
            .into_iter()
            .filter_map(|(y, names)| {
                let total: u32 = names.values().sum();
                let (name, n) = names.into_iter().max_by_key(|&(name, n)| (n, name))?;
                (total >= MIN_SAMPLES && n as f32 >= SHARE * total as f32).then(|| (y, name.to_owned()))
            })
            .collect();
        Self { by_y }
    }

    pub fn at(&self, y: i32) -> Option<&str> {
        self.by_y.get(&y).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_flat_surfaces_agreement_and_samples() {
        let flat = || (0..100).flat_map(|_| [(-64, "bedrock"), (-63, "dirt"), (-61, "grass_block")]);
        let mixed = (0..100).map(|i| (60, if i % 3 == 0 { "sand" } else { "dirt" }));
        let few = (0..10).map(|_| (70, "stone"));
        let surfaces: Vec<i32> = (0..100).map(|i| if i < 80 { -61 } else { 60 + i }).collect();
        let t = Typical::learn(&surfaces, flat().chain(mixed).chain(few));
        assert_eq!(t.at(-63), Some("dirt"));
        assert_eq!(t.at(-61), Some("grass_block"));
        assert_eq!(t.at(60), None, "two thirds is not agreement");
        assert_eq!(t.at(70), None, "too few samples");

        let hilly: Vec<i32> = (0..100).map(|i| 50 + i % 20).collect();
        assert_eq!(Typical::learn(&hilly, flat()).at(-63), None, "surfaces at many heights: not a flat world");
    }
}
