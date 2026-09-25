//! Does a site match a pack? Matching is by texture *name* + geometry, so retextures are harmless, but
//! textures the pack never saw mean a different Minecraft version, mods, or a resource pack with new
//! models — blocks using them cannot be recognised.

use std::collections::{BTreeMap, HashSet};

use crate::Pack;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Ok,
    /// Usable, but some blocks will stay unrecognised.
    Warn,
    /// Too different: the reconstruction would be mostly wrong.
    Fail,
}

#[derive(Debug)]
pub struct Compat {
    pub site_version: Option<String>,
    pub pack_version: String,
    pub known: usize,
    pub total: usize,
    /// Unknown textures per namespace (`minecraft`, mod ids, pack namespaces).
    pub unknown_by_namespace: BTreeMap<String, Vec<String>>,
}

/// Share of site textures that must be known for `Ok` / to not `Fail`.
const OK_COVERAGE: f64 = 0.999;
const FAIL_COVERAGE: f64 = 0.9;

pub fn check(pack: &Pack, site_version: Option<&str>, site_textures: &[String]) -> Compat {
    let known_set: HashSet<&str> = pack.site_textures.iter().map(String::as_str).collect();
    let mut unknown_by_namespace: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut known = 0;
    for t in site_textures {
        if known_set.contains(t.as_str()) {
            known += 1;
        } else {
            let ns = t.split_once(':').map_or("?", |(ns, _)| ns).to_owned();
            unknown_by_namespace.entry(ns).or_default().push(t.clone());
        }
    }
    Compat {
        site_version: site_version.map(str::to_owned),
        pack_version: pack.meta.bluemap_version.clone(),
        known,
        total: site_textures.len(),
        unknown_by_namespace,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{Meta, Pack};

    fn pack(textures: &[&str]) -> Pack {
        Pack {
            meta: Meta {
                mc_version: "26.3".into(),
                data_version: 5023,
                bluemap_version: "5.27".into(),
                compact_palette: true,
                created_unix: 0,
            },
            library: bmr_invert::Library::from_entries(Vec::new(), 5023),
            registry: Arc::new(bmr_world::BlockRegistry::from_blocks([])),
            template: Vec::new(),
            site_textures: textures.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn names(n: usize, prefix: &str) -> Vec<String> {
        (0..n).map(|i| format!("{prefix}{i}")).collect()
    }

    #[test]
    fn identical_site_is_ok() {
        let known = names(100, "minecraft:block/t");
        let p = pack(&known.iter().map(String::as_str).collect::<Vec<_>>());
        let c = check(&p, Some("5.27"), &known);
        assert_eq!(c.verdict(), Verdict::Ok);
        assert_eq!(c.explain().len(), 1);
    }

    #[test]
    fn a_few_mod_textures_warn_and_are_attributed() {
        let known = names(100, "minecraft:block/t");
        let p = pack(&known.iter().map(String::as_str).collect::<Vec<_>>());
        let mut site = known.clone();
        site.extend(names(3, "create:block/cog"));
        let c = check(&p, Some("5.26"), &site);
        assert_eq!(c.verdict(), Verdict::Warn);
        let text = c.explain().join("\n");
        assert!(text.contains("`create:`") && text.contains("mod") && text.contains("BlueMap 5.26"), "{text}");
    }

    #[test]
    fn mostly_unknown_fails() {
        let p = pack(&["minecraft:block/stone"]);
        let c = check(&p, None, &names(50, "minecraft:block/new"));
        assert_eq!(c.verdict(), Verdict::Fail);
    }
}

impl Compat {
    pub fn coverage(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.known as f64 / self.total as f64 }
    }

    pub fn verdict(&self) -> Verdict {
        match self.coverage() {
            c if c >= OK_COVERAGE => Verdict::Ok,
            c if c >= FAIL_COVERAGE => Verdict::Warn,
            _ => Verdict::Fail,
        }
    }

    /// Human explanation, one line per finding.
    pub fn explain(&self) -> Vec<String> {
        let mut out = vec![format!(
            "textures: {}/{} known to the pack ({:.1}%)",
            self.known,
            self.total,
            100.0 * self.coverage()
        )];
        match &self.site_version {
            Some(v) if *v != self.pack_version => out.push(format!(
                "site runs BlueMap {v}, pack was built with {} (model rendering may differ slightly)",
                self.pack_version
            )),
            None => out.push("site does not publish its BlueMap version".into()),
            _ => {}
        }
        for (ns, list) in &self.unknown_by_namespace {
            let why = if ns == "minecraft" {
                "a different Minecraft version, or a resource pack adding textures"
            } else {
                "a mod or a resource pack with its own namespace"
            };
            let sample: Vec<&str> = list.iter().take(4).map(String::as_str).collect();
            out.push(format!("{} unknown `{ns}:` textures → {why} (e.g. {})", list.len(), sample.join(", ")));
        }
        out
    }
}
