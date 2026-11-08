//! What a dimension looks like to the fill: its build height, how BlueMap renders it by default (sites do
//! not publish their map config), and the block hidden volumes default to.

use bmr_world::Dimension;

use crate::gradient::Gradients;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Overworld,
    Nether,
    End,
}

#[derive(Debug, Clone, Copy)]
pub struct Profile {
    pub kind: Kind,
    pub min_y: i32,
    pub max_y: i32,
    /// BlueMap `remove-caves-below-y`: below it, dark air also loses its faces.
    pub cave_y: i32,
    /// Heights BlueMap's `render-mask` leaves out: drawn as if empty, so faces next to it say nothing.
    pub mask: Option<(i32, i32)>,
    /// Inside the mask, above open space: rock from this height up (the nether roof's usual underside;
    /// measured on the nether fixture, where it gets 88% of those cells right vs ~55% for all rock).
    pub roof_from: i32,
    /// The world seed's exact bedrock and deepslate (`with_seed`); else the majority by height.
    pub gradients: Option<Gradients>,
}

impl Profile {
    /// BlueMap's default map for the dimension (overworld.conf / nether.conf / end.conf); a datapack dimension
    /// gets the overworld's.
    pub fn for_dimension(dimension: impl Into<Dimension>) -> Self {
        match dimension.into() {
            Dimension::Nether => Self {
                kind: Kind::Nether,
                min_y: 0,
                max_y: 255,
                cave_y: -10_000,
                mask: Some((90, 127)),
                roof_from: 108,
                gradients: None,
            },
            Dimension::End => Self {
                kind: Kind::End,
                min_y: 0,
                max_y: 255,
                cave_y: -10_000,
                mask: None,
                roof_from: 0,
                gradients: None,
            },
            Dimension::Overworld | Dimension::Other(_) => Self {
                kind: Kind::Overworld,
                min_y: -64,
                max_y: 319,
                cave_y: 55,
                mask: None,
                roof_from: 0,
                gradients: None,
            },
        }
    }

    /// Call after the heights are final: the bedrock floor gradient starts at `min_y`.
    pub fn with_seed(self, seed: i64) -> Self {
        Self { gradients: Some(Gradients::new(seed, &self)), ..self }
    }

    /// `default_block`, exact per cell when the seed is known.
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> &'static str {
        self.gradients.map_or_else(|| self.default_block(y), |g| g.block(x, y, z))
    }

    pub fn masked(&self, y: i32) -> bool {
        self.mask.is_some_and(|(lo, hi)| (lo..=hi).contains(&y))
    }

    /// Vanilla majority by height. Overworld: bedrock floor band (100/80/60% at min_y+0/1/2), deepslate up
    /// to y=3 (stone↔deepslate blends over y 0..8), stone above. Nether: bedrock floor (y 0..2) and roof
    /// (y 125..127, the majority of its 123..127 gradient), netherrack between. End: end stone, no bedrock.
    pub fn default_block(&self, y: i32) -> &'static str {
        match self.kind {
            Kind::Overworld if y <= self.min_y + 2 => "minecraft:bedrock",
            Kind::Overworld if y <= 3 => "minecraft:deepslate",
            Kind::Overworld => "minecraft:stone",
            Kind::Nether if y <= self.min_y + 2 || (125..=127).contains(&y) => "minecraft:bedrock",
            Kind::Nether => "minecraft:netherrack",
            Kind::End => "minecraft:end_stone",
        }
    }

    /// Every block `default_block` can return.
    pub fn default_blocks(&self) -> &'static [&'static str] {
        match self.kind {
            Kind::Overworld => &["minecraft:bedrock", "minecraft:deepslate", "minecraft:stone"],
            Kind::Nether => &["minecraft:bedrock", "minecraft:netherrack"],
            Kind::End => &["minecraft:end_stone"],
        }
    }

    /// Whether the world floor is bedrock (the end has void below its islands).
    pub fn bedrock_floor(&self) -> bool {
        self.kind != Kind::End
    }
}
