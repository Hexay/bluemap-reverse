//! Vanilla `random_spread` structure sets, 1.18.2 → 26.x (the server jar's `worldgen/structure_set/*.json`).

use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spread {
    Linear,
    Triangular,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reducer {
    /// pillager outposts: 1-in-n from a chunk-mixed seed
    LegacyType1 { one_in: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructureSet {
    pub name: &'static str,
    pub spacing: i32,
    pub separation: i32,
    pub salt: i32,
    pub spread: Spread,
    pub reducer: Option<Reducer>,
    since: (u32, u32, u32),
}

impl StructureSet {
    /// Range of the in-region offset per axis.
    pub fn range(&self) -> i32 {
        self.spacing - self.separation
    }
}

const fn set(name: &'static str, spacing: i32, separation: i32, salt: i32, spread: Spread) -> StructureSet {
    StructureSet { name, spacing, separation, salt, spread, reducer: None, since: (1, 18, 2) }
}

const fn since(s: StructureSet, v: (u32, u32, u32)) -> StructureSet {
    StructureSet { since: v, ..s }
}

use Spread::{Linear, Triangular};

/// Surface-relevant sets; strongholds (concentric rings), mineshafts, buried treasure and nether fossils are not
/// observable from a map render.
const VANILLA: &[StructureSet] = &[
    set("villages", 34, 8, 10387312, Linear),
    set("desert_pyramids", 32, 8, 14357617, Linear),
    set("igloos", 32, 8, 14357618, Linear),
    set("jungle_temples", 32, 8, 14357619, Linear),
    set("swamp_huts", 32, 8, 14357620, Linear),
    StructureSet {
        reducer: Some(Reducer::LegacyType1 { one_in: 5 }),
        ..set("pillager_outposts", 32, 8, 165745296, Linear)
    },
    set("ocean_monuments", 32, 5, 10387313, Triangular),
    set("woodland_mansions", 80, 20, 10387319, Triangular),
    set("shipwrecks", 24, 4, 165745295, Linear),
    set("ocean_ruins", 20, 8, 14357621, Linear),
    set("ruined_portals", 40, 15, 34222645, Linear),
    since(set("ancient_cities", 24, 8, 20083232, Linear), (1, 19, 0)),
    since(set("trail_ruins", 34, 8, 83469867, Linear), (1, 20, 0)),
    since(set("trial_chambers", 34, 12, 94251327, Linear), (1, 21, 0)),
    since(set("abandoned_camp", 37, 8, 91231127, Linear), (26, 3, 0)),
    set("nether_complexes", 27, 4, 30084232, Linear),
    set("end_cities", 20, 11, 10387313, Triangular),
];

/// "1.21.4" → (1, 21, 4); "26.3" → (26, 3, 0); "1.20-pre1" → (1, 20, 0). Year-numbered versions compare above
/// 1.x. A missing part is 0; a part without leading digits ("1.x") is `None`.
pub fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let leading = |p: &str| p[..p.find(|c: char| !c.is_ascii_digit()).unwrap_or(p.len())].parse::<u32>().ok();
    let mut it = v.split('.');
    let major = leading(it.next()?)?;
    let mut part = || it.next().map_or(Some(0), leading);
    Some((major, part()?, part()?))
}

/// The sets that exist in `mc`, or `None` for versions before 1.18.2 (other spacings and algorithms).
pub fn vanilla(mc: &str) -> Option<Vec<StructureSet>> {
    let v = parse_version(mc)?;
    if v.cmp(&(1, 18, 2)) == Ordering::Less {
        return None;
    }
    Some(VANILLA.iter().filter(|s| s.since <= v).copied().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_gates() {
        let names = |mc| vanilla(mc).unwrap().iter().map(|s| s.name).collect::<Vec<_>>();
        assert!(!names("26.2").contains(&"abandoned_camp"));
        assert!(names("26.3").contains(&"abandoned_camp"));
        assert!(!names("1.18.2").contains(&"ancient_cities"));
        assert!(vanilla("1.17.1").is_none());
        assert!(names("26.3-snapshot").contains(&"abandoned_camp"));
        assert!(names("1.20-pre1").contains(&"trail_ruins"));
    }

    #[test]
    fn parse_version_parts() {
        assert_eq!(parse_version("1.21.4"), Some((1, 21, 4)));
        assert_eq!(parse_version("26.3"), Some((26, 3, 0)));
        assert_eq!(parse_version("26.3-snapshot"), Some((26, 3, 0)));
        assert_eq!(parse_version("1.20-pre1"), Some((1, 20, 0)));
        assert_eq!(parse_version("1.20.1-rc1"), Some((1, 20, 1)));
        assert_eq!(parse_version("1.x"), None);
        assert_eq!(parse_version(""), None);
    }

    /// Cross-check against the server jar's data when a local toolchain has extracted it (tools/setup.py).
    #[test]
    fn matches_jar_data() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../work/data/structure_set-26.3");
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let sets = vanilla("26.3").unwrap();
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().trim_end_matches(".json").to_string();
            let Some(s) = sets.iter().find(|s| s.name == name) else { continue };
            let json: serde_json::Value = serde_json::from_slice(&std::fs::read(e.path()).unwrap()).unwrap();
            let p = &json["placement"];
            assert_eq!(p["spacing"], s.spacing, "{name}");
            assert_eq!(p["separation"], s.separation, "{name}");
            assert_eq!(p["salt"], s.salt, "{name}");
            let triangular = p["spread_type"].as_str().is_some_and(|t| t.ends_with("triangular"));
            assert_eq!(triangular, s.spread == Triangular, "{name}");
        }
    }
}
