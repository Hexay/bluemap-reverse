//! Which states the tiles can tell apart. Keys (faces, texture) are compared exactly; UVs and block light
//! only where they are deterministic: BlueMap rotates many textures by position hash (stone, grass, kelp),
//! so debug-world UVs differ between e.g. stone and infested_stone by chance, and a light source two
//! cells away lights debug-world neighbours.

use rustc_hash::FxHashMap;

use crate::library::{Entry, Library};

/// Properties that turn or mirror a model (and with it its UVs).
const ORIENTATION: [&str; 11] =
    ["facing", "rotation", "hinge", "axis", "orientation", "face", "attachment", "half", "open", "shape", "part"];

fn prop<'a>(e: &'a Entry, key: &str) -> Option<&'a str> {
    e.state.properties.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// UVs can separate these candidates: they differ in how the model is turned. (Randomly rotated blocks
/// have no such property, so stone vs infested_stone or kelp ages never compare UVs.)
pub fn uv_decides(lib: &Library, ids: &[usize]) -> bool {
    let first = &lib.entries[ids[0]];
    ORIENTATION.iter().any(|p| ids.iter().any(|&i| prop(&lib.entries[i], p) != prop(first, p)))
}

/// Block light can separate these candidates: they differ in being lit.
pub fn light_decides(lib: &Library, ids: &[usize]) -> bool {
    let first = &lib.entries[ids[0]];
    ids.iter().any(|&i| prop(&lib.entries[i], "lit") != prop(first, "lit"))
}

impl Library {
    /// Groups of 2+ states BlueMap renders identically: same faces, overhang, liquid and tint, and UVs or
    /// light either equal or not deciding (see module doc). No tile tells a group's members apart.
    pub fn lookalikes(&self) -> Vec<Vec<usize>> {
        let mut base: FxHashMap<_, Vec<usize>> = FxHashMap::default();
        for (i, e) in self.entries.iter().enumerate() {
            let mut overhang = e.overhang.clone();
            overhang.sort();
            base.entry((&e.sig, overhang, e.liquid, e.tint)).or_default().push(i);
        }
        let mut out = Vec::new();
        for ids in base.into_values().filter(|g| g.len() > 1) {
            // members join the first class they cannot be told apart from
            let mut classes: Vec<Vec<usize>> = Vec::new();
            for id in ids {
                match classes.iter_mut().find(|c| !self.tell_apart(c[0], id)) {
                    Some(c) => c.push(id),
                    None => classes.push(vec![id]),
                }
            }
            out.extend(classes.into_iter().filter(|c| c.len() > 1));
        }
        out
    }

    /// Two entries with the same faces render distinguishably (reliable UV or light difference).
    pub fn tell_apart(&self, a: usize, b: usize) -> bool {
        let (x, y) = (&self.entries[a], &self.entries[b]);
        (uv_decides(self, &[a, b]) && x.uvs != y.uvs) || (light_decides(self, &[a, b]) && x.light != y.light)
    }
}

#[cfg(test)]
#[path = "lookalike_tests.rs"]
mod tests;
