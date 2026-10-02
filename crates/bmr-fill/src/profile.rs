//! What a dimension looks like to the fill: its build height, how BlueMap renders it by default (sites do
//! not publish their map config), and the block hidden volumes default to.

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
}

impl Profile {
    /// BlueMap's default map for the dimension (overworld.conf / nether.conf / end.conf).
    pub fn for_dimension(id: &str) -> Self {
        match id {
            "minecraft:the_nether" => Self {
                kind: Kind::Nether,
                min_y: 0,
                max_y: 255,
                cave_y: -10_000,
                mask: Some((90, 127)),
                roof_from: 108,
            },
            "minecraft:the_end" => {
                Self { kind: Kind::End, min_y: 0, max_y: 255, cave_y: -10_000, mask: None, roof_from: 0 }
            }
            _ => Self { kind: Kind::Overworld, min_y: -64, max_y: 319, cave_y: 55, mask: None, roof_from: 0 },
        }
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
