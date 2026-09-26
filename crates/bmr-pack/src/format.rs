//! On-disk pack layout: `MAGIC` + u32 LE header length + xz(postcard(`Header`)) + xz(postcard(`Body`)).
//! The header is separate so choosing among many packs reads only headers. The body is deduplicated:
//! states are (block, ordinal) into the registry, and face keys and whole signatures are stored once and
//! referenced by index (many states render identically). Texture ids are process-local, so keys index the
//! body's own texture-name table. `default_distance`/`full_cube` are recomputed on load.

use std::collections::HashMap;

use anyhow::{Context, Result};
use bmr_invert::face::{FaceKey, Liquid, Tex, Uv};
use bmr_invert::library::{Entry, default_distance, is_full_cube};
use bmr_world::{BlockInfo, BlockState};
use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8; 8] = b"BMRPACK\0";
/// Bump on any change to the structs below.
pub const FORMAT: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub mc_version: String,
    pub data_version: i32,
    pub bluemap_version: String,
    /// Palettes are written in 26.3's compact form (else legacy `{Name, Properties}`).
    pub compact_palette: bool,
    pub created_unix: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Header {
    pub format: u32,
    pub meta: Meta,
    /// Every texture of the library map (index = material index there): the compatibility baseline.
    pub site_textures: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Body {
    pub key_textures: Vec<String>,
    pub keys: Vec<KeyDto>,
    /// Unique signatures as indices into `keys`.
    pub sigs: Vec<Vec<u32>>,
    /// Unique UV lists (parallel to a signature).
    pub uv_sets: Vec<Vec<Uv>>,
    pub entries: Vec<EntryDto>,
    pub registry: Vec<(String, BlockInfo)>,
    pub template: Vec<(String, Vec<u8>)>,
}

#[derive(Serialize, Deserialize)]
pub struct EntryDto {
    /// Index into `Body::registry`.
    block: u32,
    /// Mixed-radix index over the block's properties in registry order.
    state: u32,
    sig: u32,
    uvs: u32,
    light: u8,
    overhang: Vec<([i32; 3], u32)>,
    /// 0 none, 1 water, 2 lava
    liquid: u8,
    tint: Option<[u8; 3]>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct KeyDto {
    tex: u32,
    tinted: bool,
    verts: [[i16; 3]; 4],
}

pub fn encode(entries: &[Entry], registry: Vec<(String, BlockInfo)>, template: Vec<(String, Vec<u8>)>) -> Result<Body> {
    let blocks: HashMap<&str, u32> = registry.iter().enumerate().map(|(i, (n, _))| (n.as_str(), i as u32)).collect();
    let mut t = Tables::default();
    let dtos = entries
        .iter()
        .map(|e| {
            let block = *blocks.get(e.state.name.as_str()).with_context(|| format!("{} not in registry", e.state.name))?;
            Ok(EntryDto {
                block,
                state: ordinal(&e.state, &registry[block as usize].1)?,
                sig: t.sig(&e.sig),
                uvs: t.uvs(&e.uvs),
                light: e.light,
                overhang: e.overhang.iter().map(|(o, k)| ([o.0, o.1, o.2], t.key(k))).collect(),
                liquid: match e.liquid {
                    None => 0,
                    Some(Liquid::Water) => 1,
                    Some(Liquid::Lava) => 2,
                },
                tint: e.tint,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Body { key_textures: t.textures, keys: t.keys, sigs: t.sigs, uv_sets: t.uv_sets, entries: dtos, registry, template })
}

/// Entries plus the registry and template, moved out of `body`.
pub fn decode(body: Body) -> (Vec<Entry>, Vec<(String, BlockInfo)>, Vec<(String, Vec<u8>)>) {
    let textures: Vec<Tex> = body.key_textures.iter().map(|n| Tex::intern(n)).collect();
    let keys: Vec<FaceKey> =
        body.keys.iter().map(|k| FaceKey { texture: textures[k.tex as usize], tinted: k.tinted, verts: k.verts }).collect();
    let entries = body
        .entries
        .into_iter()
        .map(|d| {
            let (name, info) = &body.registry[d.block as usize];
            let state = from_ordinal(name, info, d.state);
            let sig: Vec<FaceKey> = body.sigs[d.sig as usize].iter().map(|&k| keys[k as usize]).collect();
            Entry {
                default_distance: default_distance(&state, info),
                full_cube: is_full_cube(&sig),
                state,
                sig,
                uvs: body.uv_sets[d.uvs as usize].clone(),
                light: d.light,
                overhang: d.overhang.into_iter().map(|(o, k)| ((o[0], o[1], o[2]), keys[k as usize])).collect(),
                liquid: match d.liquid {
                    1 => Some(Liquid::Water),
                    2 => Some(Liquid::Lava),
                    _ => None,
                },
                tint: d.tint,
            }
        })
        .collect();
    (entries, body.registry, body.template)
}

fn ordinal(state: &BlockState, info: &BlockInfo) -> Result<u32> {
    info.properties.iter().try_fold(0u32, |acc, (prop, values)| {
        let value = state.properties.iter().find(|(p, _)| p == prop).map(|(_, v)| v);
        let i = value.and_then(|v| values.iter().position(|x| x == v));
        let i = i.with_context(|| format!("{}: {prop}={value:?} not in registry", state.name))?;
        Ok(acc * values.len() as u32 + i as u32)
    })
}

fn from_ordinal(name: &str, info: &BlockInfo, mut ord: u32) -> BlockState {
    let mut props = Vec::with_capacity(info.properties.len());
    for (prop, values) in info.properties.iter().rev() {
        let n = values.len() as u32;
        props.push((prop.clone(), values[(ord % n) as usize].clone()));
        ord /= n;
    }
    BlockState::new(name.to_owned(), props)
}

/// Interns texture names, face keys and signatures while encoding.
#[derive(Default)]
struct Tables {
    textures: Vec<String>,
    tex_index: HashMap<Tex, u32>,
    keys: Vec<KeyDto>,
    key_index: HashMap<FaceKey, u32>,
    sigs: Vec<Vec<u32>>,
    sig_index: HashMap<Vec<u32>, u32>,
    uv_sets: Vec<Vec<Uv>>,
    uv_index: HashMap<Vec<Uv>, u32>,
}

impl Tables {
    fn key(&mut self, k: &FaceKey) -> u32 {
        if let Some(&i) = self.key_index.get(k) {
            return i;
        }
        let tex = *self.tex_index.entry(k.texture).or_insert_with(|| {
            self.textures.push(k.texture.name().to_string());
            (self.textures.len() - 1) as u32
        });
        self.keys.push(KeyDto { tex, tinted: k.tinted, verts: k.verts });
        let i = (self.keys.len() - 1) as u32;
        self.key_index.insert(*k, i);
        i
    }

    fn sig(&mut self, sig: &[FaceKey]) -> u32 {
        let ids: Vec<u32> = sig.iter().map(|k| self.key(k)).collect();
        let next = self.sigs.len() as u32;
        *self.sig_index.entry(ids).or_insert_with_key(|ids| {
            self.sigs.push(ids.clone());
            next
        })
    }

    fn uvs(&mut self, uvs: &[Uv]) -> u32 {
        let next = self.uv_sets.len() as u32;
        *self.uv_index.entry(uvs.to_vec()).or_insert_with_key(|uvs| {
            self.uv_sets.push(uvs.clone());
            next
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn roundtrip_is_lossless() {
        let stairs_props: Vec<(String, Vec<String>)> = [("facing", ["north", "south"]), ("half", ["top", "bottom"]), ("waterlogged", ["true", "false"])]
            .iter()
            .map(|(p, vs)| (p.to_string(), vs.iter().map(|v| v.to_string()).collect()))
            .collect();
        let registry = vec![
            ("minecraft:oak_stairs".to_string(), BlockInfo {
                properties: stairs_props,
                default: owned(&[("facing", "north"), ("half", "bottom"), ("waterlogged", "false")]),
            }),
            ("minecraft:stone".to_string(), BlockInfo { properties: vec![], default: vec![] }),
        ];
        let key = |t: &str, v: i16| FaceKey { texture: Tex::intern(t), tinted: false, verts: [[0, 0, 0], [v, 0, 0], [v, v, 0], [0, v, 0]] };
        let stairs = BlockState::new("minecraft:oak_stairs".into(), owned(&[("facing", "south"), ("half", "top"), ("waterlogged", "true")]));
        let stone = BlockState::new("minecraft:stone".into(), vec![]);
        let entry = |state, sig: Vec<FaceKey>, liquid, overhang, tint, default_distance| {
            let uvs = (0..sig.len()).map(|i| [[i as i16, 0], [0, 1], [1, 1], [1, 0]]).collect();
            Entry { state, sig, uvs, light: 3, liquid, overhang, tint, default_distance, full_cube: false }
        };
        let entries = vec![
            entry(stone.clone(), vec![key("block/stone", 64)], None, vec![], None, 0),
            entry(stairs, vec![key("block/oak_planks", 32), key("block/stone", 64)], Some(Liquid::Water), vec![((0, 1, 0), key("block/oak_planks", 16))], Some([1, 2, 3]), 3),
            entry(stone, vec![key("block/stone", 64)], None, vec![], None, 0),
        ];
        let body = encode(&entries, registry, vec![]).unwrap();
        assert_eq!(body.sigs.len(), 2, "identical signatures are stored once");
        let (back, _, _) = decode(body);
        let view = |e: &Entry| {
            (e.state.clone(), e.sig.clone(), e.uvs.clone(), e.light, e.liquid, e.overhang.clone(), e.tint, e.default_distance, e.full_cube)
        };
        assert_eq!(entries.iter().map(view).collect::<Vec<_>>(), back.iter().map(view).collect::<Vec<_>>());
    }
}
