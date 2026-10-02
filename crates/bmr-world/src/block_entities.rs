//! Block → block-entity type. The server does not recreate missing block entities on load, and a chest
//! without one is invisible in-game, so the writer emits a minimal `{id,x,y,z}` for each.
//! The vanilla reports list the types (registries.json) but not the mapping; the test checks coverage.

/// BE types whose id equals the block name.
const SAME_NAME: &[&str] = &[
    "barrel",
    "beacon",
    "beehive",
    "bell",
    "blast_furnace",
    "brewing_stand",
    "calibrated_sculk_sensor",
    "campfire",
    "chest",
    "chiseled_bookshelf",
    "command_block",
    "comparator",
    "conduit",
    "crafter",
    "creaking_heart",
    "daylight_detector",
    "decorated_pot",
    "dispenser",
    "dropper",
    "enchanting_table",
    "end_gateway",
    "end_portal",
    "ender_chest",
    "furnace",
    "hopper",
    "jigsaw",
    "jukebox",
    "lectern",
    "potent_sulfur",
    "sculk_catalyst",
    "sculk_sensor",
    "sculk_shrieker",
    "smoker",
    "structure_block",
    "test_block",
    "test_instance_block",
    "trapped_chest",
    "trial_spawner",
    "vault",
];

/// Block-entity id (without namespace) for a block name (with namespace), if it has one.
/// `moving_piston` is deliberately excluded: it only exists mid-animation.
pub fn block_entity_type(block: &str) -> Option<&'static str> {
    let name = block.strip_prefix("minecraft:")?;
    if let Some(t) = SAME_NAME.iter().find(|t| **t == name) {
        return Some(t);
    }
    Some(match name {
        "bee_nest" => "beehive",
        "soul_campfire" => "campfire",
        "chain_command_block" | "repeating_command_block" => "command_block",
        "spawner" => "mob_spawner",
        "suspicious_sand" | "suspicious_gravel" => "brushable_block",
        "piston_head" => return None,
        n if n.ends_with("_hanging_sign") => "hanging_sign",
        n if n.ends_with("_sign") => "sign",
        n if n.ends_with("_banner") => "banner",
        n if n.ends_with("_skull") || n.ends_with("_head") => "skull",
        n if n.ends_with("shulker_box") => "shulker_box",
        n if n.ends_with("_shelf") => "shelf",
        n if n.ends_with("copper_golem_statue") => "copper_golem_statue",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    const REPORTS: &str = "../../work/data/reports-26.3/reports";

    #[test]
    fn known_mappings() {
        assert_eq!(block_entity_type("minecraft:oak_wall_sign"), Some("sign"));
        assert_eq!(block_entity_type("minecraft:oak_wall_hanging_sign"), Some("hanging_sign"));
        assert_eq!(block_entity_type("minecraft:player_wall_head"), Some("skull"));
        assert_eq!(block_entity_type("minecraft:piston_head"), None);
        assert_eq!(block_entity_type("minecraft:stone"), None);
        assert_eq!(block_entity_type("minecraft:oak_stairs"), None);
    }

    /// Every registered BE type (except piston) is produced by at least one registered block.
    #[test]
    fn covers_every_registered_type() {
        let dir = Path::new(REPORTS);
        if !dir.exists() {
            eprintln!("skipped: {REPORTS} missing (run tools/setup.py)");
            return;
        }
        let read =
            |f: &str| -> serde_json::Value { serde_json::from_slice(&std::fs::read(dir.join(f)).unwrap()).unwrap() };
        let registries = read("registries.json");
        let types: BTreeSet<String> = registries["minecraft:block_entity_type"]["entries"]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.trim_start_matches("minecraft:").to_owned())
            .filter(|t| t != "piston")
            .collect();
        let blocks = read("blocks.json");
        let produced: BTreeSet<String> =
            blocks.as_object().unwrap().keys().filter_map(|b| block_entity_type(b).map(str::to_owned)).collect();
        let missing: Vec<_> = types.difference(&produced).collect();
        let bogus: Vec<_> = produced.difference(&types).collect();
        assert!(missing.is_empty() && bogus.is_empty(), "unmapped types {missing:?}, unknown types {bogus:?}");
    }
}
