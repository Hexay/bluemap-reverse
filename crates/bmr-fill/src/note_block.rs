//! Note blocks render the same for every instrument; the game sets `instrument` from a mob head on top,
//! else from the block below (vanilla `BlockBehaviour.Properties.instrument`, grouped here by name).

use rustc_hash::FxHashMap;

use bmr_invert::face::Cell;
use bmr_world::BlockState;

use crate::rules::{prop, set_prop};

const HEADS: [(&str, &str); 7] = [
    ("zombie_head", "zombie"),
    ("skeleton_skull", "skeleton"),
    ("creeper_head", "creeper"),
    ("dragon_head", "dragon"),
    ("wither_skeleton_skull", "wither_skeleton"),
    ("piglin_head", "piglin"),
    ("player_head", "custom_head"),
];

/// Exact block names (without `minecraft:`) with their own instrument.
const EXACT: [(&str, &str); 15] = [
    ("clay", "flute"),
    ("gold_block", "bell"),
    ("packed_ice", "chime"),
    ("bone_block", "xylophone"),
    ("iron_block", "iron_xylophone"),
    ("soul_sand", "cow_bell"),
    ("pumpkin", "didgeridoo"),
    ("carved_pumpkin", "didgeridoo"),
    ("jack_o_lantern", "didgeridoo"),
    ("emerald_block", "bit"),
    ("hay_block", "banjo"),
    ("glowstone", "pling"),
    ("sea_lantern", "hat"),
    ("beacon", "hat"),
    ("note_block", "bass"),
];

/// (name contains, instrument), first match wins.
const PATTERNS: [(&str, &str); 41] = [
    ("sandstone", "basedrum"),
    ("redstone_ore", "basedrum"),
    ("redstone", "harp"),
    ("concrete_powder", "snare"),
    ("suspicious_", "snare"),
    ("sand", "snare"),
    ("gravel", "snare"),
    ("glass", "hat"),
    ("wool", "guitar"),
    ("weathered_c", "trumpet_weathered"),
    ("oxidized_c", "trumpet_oxidized"),
    ("exposed_c", "trumpet_exposed"),
    ("copper_block", "trumpet"),
    ("cut_copper", "trumpet"),
    ("planks", "bass"),
    ("_log", "bass"),
    ("_wood", "bass"),
    ("_stem", "bass"),
    ("_hyphae", "bass"),
    ("bamboo", "bass"),
    ("bookshelf", "bass"),
    ("mushroom_block", "bass"),
    ("chest", "bass"),
    ("barrel", "bass"),
    ("table", "bass"),
    ("stone", "basedrum"),
    ("cobble", "basedrum"),
    ("deepslate", "basedrum"),
    ("brick", "basedrum"),
    ("ore", "basedrum"),
    ("terracotta", "basedrum"),
    ("concrete", "basedrum"),
    ("netherrack", "basedrum"),
    ("nylium", "basedrum"),
    ("obsidian", "basedrum"),
    ("quartz", "basedrum"),
    ("prismarine", "basedrum"),
    ("purpur", "basedrum"),
    ("basalt", "basedrum"),
    ("tuff", "basedrum"),
    ("bedrock", "basedrum"),
];

/// Returns the number of note blocks whose instrument changed.
pub fn instruments(blocks: &mut FxHashMap<Cell, BlockState>) -> usize {
    let updates: Vec<(Cell, &'static str)> = blocks
        .iter()
        .filter(|(_, s)| s.name == "minecraft:note_block")
        .map(|(&(x, y, z), _)| {
            let above = blocks.get(&(x, y + 1, z)).map(|b| short(&b.name));
            let below = blocks.get(&(x, y - 1, z)).map(|b| short(&b.name));
            let instrument = above.and_then(head).or_else(|| below.map(by_material)).unwrap_or("harp");
            ((x, y, z), instrument)
        })
        .filter(|(c, i)| prop(&blocks[c], "instrument") != Some(i))
        .collect();
    for (c, i) in &updates {
        set_prop(blocks.get_mut(c).expect("note block"), "instrument", i);
    }
    updates.len()
}

fn short(name: &str) -> &str {
    name.trim_start_matches("minecraft:")
}

fn head(name: &str) -> Option<&'static str> {
    HEADS.iter().find(|(h, _)| name == *h).map(|(_, i)| *i)
}

fn by_material(name: &str) -> &'static str {
    let name = name.trim_start_matches("waxed_");
    EXACT
        .iter()
        .find(|(n, _)| *n == name)
        .or_else(|| PATTERNS.iter().find(|(p, _)| name.contains(p)))
        .map_or("harp", |(_, i)| *i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_table() {
        assert_eq!(by_material("oak_planks"), "bass");
        assert_eq!(by_material("stone_bricks"), "basedrum");
        assert_eq!(by_material("red_sand"), "snare");
        assert_eq!(by_material("white_stained_glass"), "hat");
        assert_eq!(by_material("waxed_weathered_cut_copper"), "trumpet_weathered");
        assert_eq!(by_material("dirt"), "harp");
        assert_eq!(by_material("cut_red_sandstone"), "basedrum");
        assert_eq!(by_material("redstone_block"), "harp");
        assert_eq!(head("creeper_head"), Some("creeper"));
    }
}
