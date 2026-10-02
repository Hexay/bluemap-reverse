//! On-disk shape of the Sponge v3 schematic and its palette/varint encoding.

use std::collections::BTreeMap;

use fastnbt::{ByteArray, IntArray};
use rustc_hash::FxHashMap;
use serde::Serialize;

use crate::chunk::BlockState;

#[derive(Serialize)]
pub(super) struct Root {
    #[serde(rename = "Schematic")]
    pub schematic: Schematic,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct Schematic {
    pub version: i32,
    pub data_version: i32,
    pub metadata: Metadata,
    /// unsigned shorts in the spec
    pub width: i16,
    pub height: i16,
    pub length: i16,
    pub offset: IntArray,
    pub blocks: Blocks,
    pub biomes: Biomes,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct Metadata {
    pub name: String,
    /// Unix milliseconds
    pub date: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct Blocks {
    pub palette: BTreeMap<String, i32>,
    pub data: ByteArray,
    pub block_entities: Vec<BlockEntity>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct Biomes {
    pub palette: BTreeMap<String, i32>,
    pub data: ByteArray,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct BlockEntity {
    pub pos: IntArray,
    pub id: String,
    pub data: BTreeMap<String, i32>,
}

/// Schematic palette id; every air variant is `minecraft:air` (0), so `non_air` agrees with trimming.
pub(super) fn block_id(palette: &mut FxHashMap<String, u32>, b: &BlockState) -> u32 {
    if b.is_air() { 0 } else { intern(palette, &b.to_string()) }
}

pub(super) fn intern(palette: &mut FxHashMap<String, u32>, key: &str) -> u32 {
    if let Some(&i) = palette.get(key) {
        return i;
    }
    let i = palette.len() as u32;
    palette.insert(key.to_owned(), i);
    i
}

pub(super) fn sorted(palette: FxHashMap<String, u32>) -> BTreeMap<String, i32> {
    palette.into_iter().map(|(k, v)| (k, v as i32)).collect()
}

/// Unsigned LEB128, 7 bits per byte, high bit = more.
pub(super) fn varints(values: &[u32]) -> ByteArray {
    let mut out = Vec::with_capacity(values.len());
    for &v in values {
        let mut v = v;
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(byte as i8);
                break;
            }
            out.push((byte | 0x80) as i8);
        }
    }
    ByteArray::new(out)
}

pub(super) fn unix_millis() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_boundaries() {
        let b = varints(&[0, 127, 128, 300]);
        let bytes: Vec<u8> = b.iter().map(|&x| x as u8).collect();
        assert_eq!(bytes, vec![0x00, 0x7f, 0x80, 0x01, 0xac, 0x02]);
    }

    #[test]
    fn air_variants_share_id_zero() {
        let mut palette = FxHashMap::default();
        palette.insert("minecraft:air".to_owned(), 0);
        let id = |p: &mut FxHashMap<String, u32>, n: &str| block_id(p, &BlockState::new(n.into(), Vec::new()));
        assert_eq!(id(&mut palette, "minecraft:cave_air"), 0);
        assert_eq!(id(&mut palette, "minecraft:void_air"), 0);
        assert_eq!(id(&mut palette, "minecraft:stone"), 1);
        assert_eq!(palette.len(), 2);
    }
}
