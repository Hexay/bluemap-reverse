//! `bmr pack diff`: does a pack add anything over another one? Decides whether a build for another BlueMap
//! version is worth shipping (versions and build time are ignored).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use bmr_invert::library::Entry;
use bmr_pack::Pack;

const SAMPLES: usize = 8;

/// Per-field counts of states present in both packs, with sample state names.
#[derive(Default)]
struct FieldDiff {
    count: usize,
    samples: Vec<String>,
}

impl FieldDiff {
    fn add(&mut self, state: &str) {
        self.count += 1;
        if self.samples.len() < SAMPLES {
            self.samples.push(state.to_owned());
        }
    }
}

pub fn diff(a_path: &Path, b_path: &Path) -> Result<()> {
    let (a, b) = (Pack::load(a_path)?, Pack::load(b_path)?);
    println!("a: Minecraft {}, BlueMap {}", a.meta.mc_version, a.meta.bluemap_version);
    println!("b: Minecraft {}, BlueMap {}", b.meta.mc_version, b.meta.bluemap_version);

    let mut identical = report_sets("site textures", &a.site_textures, &b.site_textures);
    let by_state = |p: &'_ Pack| -> BTreeMap<String, usize> {
        p.library.entries.iter().enumerate().map(|(i, e)| (e.state.to_string(), i)).collect()
    };
    let (sa, sb) = (by_state(&a), by_state(&b));
    let names = |m: &BTreeMap<String, usize>| m.keys().cloned().collect::<Vec<_>>();
    identical &= report_sets("states", &names(&sa), &names(&sb));

    let mut fields: BTreeMap<&str, FieldDiff> = BTreeMap::new();
    for (state, &i) in &sa {
        let Some(&j) = sb.get(state) else { continue };
        for field in differing_fields(&a.library.entries[i], &b.library.entries[j]) {
            fields.entry(field).or_default().add(state);
        }
    }
    identical &= fields.is_empty();
    for (field, d) in &fields {
        println!("{field}: {} states differ, e.g. {}", d.count, d.samples.join(", "));
    }

    // the list is ordered by frequency, so equal counts land in any order
    let tints = |p: &'_ Pack| p.biome_tints.iter().map(|t| (t.biome.clone(), t.tints)).collect::<BTreeMap<_, _>>();
    let (ta, tb) = (tints(&a), tints(&b));
    let tints_equal = ta == tb;
    for (biome, t) in ta.iter().filter(|(k, v)| tb.get(*k) != Some(v)).take(SAMPLES) {
        println!("  {biome}: {t:?} / {:?}", tb.get(biome));
    }
    let template_equal = a.template == b.template;
    println!("biome tints: {}", if tints_equal { "equal" } else { "differ" });
    println!("template: {}", if template_equal { "equal" } else { "differ" });
    identical &= tints_equal;
    println!("{}", if identical { "IDENTICAL (template aside)" } else { "DIFFERENT" });
    Ok(())
}

fn differing_fields(a: &Entry, b: &Entry) -> Vec<&'static str> {
    [
        ("signature", a.sig != b.sig),
        ("uvs", a.uvs != b.uvs),
        ("light", a.light != b.light),
        ("liquid", a.liquid != b.liquid),
        ("overhang", a.overhang != b.overhang),
        ("tint", a.tint != b.tint),
    ]
    .into_iter()
    .filter_map(|(name, differs)| differs.then_some(name))
    .collect()
}

/// Prints the set difference; true when equal.
fn report_sets(what: &str, a: &[String], b: &[String]) -> bool {
    let (a, b): (BTreeSet<&String>, BTreeSet<&String>) = (a.iter().collect(), b.iter().collect());
    let sample = |d: Vec<&&String>| d.iter().take(SAMPLES).map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
    let (only_a, only_b): (Vec<_>, Vec<_>) = (a.difference(&b).collect(), b.difference(&a).collect());
    println!(
        "{what}: {} / {}, only in a: {} ({}), only in b: {} ({})",
        a.len(),
        b.len(),
        only_a.len(),
        sample(only_a.clone()),
        only_b.len(),
        sample(only_b.clone())
    );
    only_a.is_empty() && only_b.is_empty()
}
