//! Process-wide texture-name interner: face keys compare and hash a `u32` instead of a string.
//! Ids depend on interning order, so they are only comparable within one process (the library and the
//! map being inverted share it); anything persisted must store names, not ids.

use std::sync::{Arc, LazyLock, RwLock};

use rustc_hash::FxHashMap;

use crate::face::Liquid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tex(u32);

#[derive(Default)]
struct Interner {
    names: Vec<Arc<str>>,
    ids: FxHashMap<Arc<str>, u32>,
}

static INTERNER: LazyLock<RwLock<Interner>> = LazyLock::new(Default::default);

static LIQUIDS: LazyLock<[(Tex, Liquid); 4]> = LazyLock::new(|| {
    [
        (Tex::intern("minecraft:block/water_still"), Liquid::Water),
        (Tex::intern("minecraft:block/water_flow"), Liquid::Water),
        (Tex::intern("minecraft:block/lava_still"), Liquid::Lava),
        (Tex::intern("minecraft:block/lava_flow"), Liquid::Lava),
    ]
});

/// Textures a model picks between by position hash, with identical geometry: a state's library entry
/// shows only the one its debug-world position drew, so they must compare equal.
const RANDOM_VARIANTS: [(&str, &str); 2] = [
    ("minecraft:block/fire_1", "minecraft:block/fire_0"),
    ("minecraft:block/soul_fire_1", "minecraft:block/soul_fire_0"),
];

impl Tex {
    pub fn intern(name: &str) -> Tex {
        let name = RANDOM_VARIANTS.iter().find(|(v, _)| *v == name).map_or(name, |(_, canonical)| canonical);
        if let Some(&id) = INTERNER.read().unwrap().ids.get(name) {
            return Tex(id);
        }
        let mut w = INTERNER.write().unwrap();
        if let Some(&id) = w.ids.get(name) {
            return Tex(id);
        }
        let id = w.names.len() as u32;
        let name: Arc<str> = Arc::from(name);
        w.names.push(name.clone());
        w.ids.insert(name, id);
        Tex(id)
    }

    pub fn name(self) -> Arc<str> {
        INTERNER.read().unwrap().names[self.0 as usize].clone()
    }

    pub fn liquid(self) -> Option<Liquid> {
        LIQUIDS.iter().find(|(t, _)| *t == self).map(|&(_, l)| l)
    }
}
