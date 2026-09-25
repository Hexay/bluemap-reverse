//! Block registry from the vanilla data report `blocks.json` (tools/setup.py generates it).

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    /// Property name → allowed values, in report order.
    pub properties: Vec<(String, Vec<String>)>,
    /// Sorted by key.
    pub default: Vec<(String, String)>,
}

pub struct BlockRegistry {
    blocks: HashMap<String, BlockInfo>,
}

#[derive(Deserialize)]
struct BlockJson {
    #[serde(default)]
    properties: indexmap_like::OrderedMap,
    states: Vec<StateJson>,
}

#[derive(Deserialize)]
struct StateJson {
    #[serde(default)]
    default: bool,
    #[serde(default)]
    properties: HashMap<String, String>,
}

impl BlockRegistry {
    pub fn load(blocks_json: &Path) -> Result<Self> {
        let bytes = std::fs::read(blocks_json).with_context(|| blocks_json.display().to_string())?;
        let raw: HashMap<String, BlockJson> = serde_json::from_slice(&bytes).context("blocks.json")?;
        let blocks = raw
            .into_iter()
            .map(|(name, b)| {
                let mut default: Vec<(String, String)> = b
                    .states
                    .into_iter()
                    .find(|s| s.default)
                    .map(|s| s.properties.into_iter().collect())
                    .unwrap_or_default();
                default.sort();
                (name, BlockInfo { properties: b.properties.0, default })
            })
            .collect();
        Ok(Self { blocks })
    }

    /// From already-parsed entries (e.g. a bmr pack).
    pub fn from_blocks(blocks: impl IntoIterator<Item = (String, BlockInfo)>) -> Self {
        Self { blocks: blocks.into_iter().collect() }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &BlockInfo)> {
        self.blocks.iter()
    }

    pub fn get(&self, name: &str) -> Option<&BlockInfo> {
        self.blocks.get(name)
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// JSON object → Vec keeping key order (serde_json's Map sorts unless `preserve_order` is enabled).
mod indexmap_like {
    use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
    use std::fmt;

    #[derive(Default)]
    pub struct OrderedMap(pub Vec<(String, Vec<String>)>);

    impl<'de> Deserialize<'de> for OrderedMap {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl<'de> Visitor<'de> for V {
                type Value = OrderedMap;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("a map of property name to values")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<OrderedMap, A::Error> {
                    let mut out = Vec::new();
                    while let Some(entry) = m.next_entry()? {
                        out.push(entry);
                    }
                    Ok(OrderedMap(out))
                }
            }
            d.deserialize_map(V)
        }
    }
}
