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
pub const FORMAT: u32 = 5;

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
    /// (biome, grass/foliage/water tint as BlueMap draws them), commonest biome first.
    pub biome_tints: Vec<(String, [Option<[u8; 3]>; 3])>,
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
            let block =
                *blocks.get(e.state.name.as_str()).with_context(|| format!("{} not in registry", e.state.name))?;
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
    Ok(Body {
        key_textures: t.textures,
        keys: t.keys,
        sigs: t.sigs,
        uv_sets: t.uv_sets,
        entries: dtos,
        registry,
        template,
        biome_tints: Vec::new(),
    })
}

pub type Decoded = (Vec<Entry>, Vec<(String, BlockInfo)>, Vec<(String, Vec<u8>)>);

/// Entries plus the registry and template, moved out of `body`. Every index is checked: a corrupt or
/// hand-made pack is an error, not a panic.
pub fn decode(body: Body) -> Result<Decoded> {
    let textures: Vec<Tex> = body.key_textures.iter().map(|n| Tex::intern(n)).collect();
    let keys: Vec<FaceKey> = body
        .keys
        .iter()
        .map(|k| Ok(FaceKey { texture: *at(&textures, k.tex, "texture")?, tinted: k.tinted, verts: k.verts }))
        .collect::<Result<_>>()?;
    let key = |k: u32| at(&keys, k, "face key").copied();
    let entries = body
        .entries
        .into_iter()
        .map(|d| {
            let (name, info) = at(&body.registry, d.block, "block")?;
            let state = from_ordinal(name, info, d.state)?;
            let sig: Vec<FaceKey> =
                at(&body.sigs, d.sig, "signature")?.iter().map(|&k| key(k)).collect::<Result<_>>()?;
            Ok(Entry {
                default_distance: default_distance(&state, info),
                full_cube: is_full_cube(&sig),
                state,
                sig,
                uvs: at(&body.uv_sets, d.uvs, "uv set")?.clone(),
                light: d.light,
                overhang: d
                    .overhang
                    .into_iter()
                    .map(|(o, k)| Ok(((o[0], o[1], o[2]), key(k)?)))
                    .collect::<Result<_>>()?,
                liquid: match d.liquid {
                    1 => Some(Liquid::Water),
                    2 => Some(Liquid::Lava),
                    _ => None,
                },
                tint: d.tint,
            })
        })
        .collect::<Result<_>>()?;
    Ok((entries, body.registry, body.template))
}

fn at<'a, T>(table: &'a [T], i: u32, what: &str) -> Result<&'a T> {
    table.get(i as usize).with_context(|| format!("corrupt pack: {what} index {i} out of range ({})", table.len()))
}

fn ordinal(state: &BlockState, info: &BlockInfo) -> Result<u32> {
    info.properties.iter().try_fold(0u32, |acc, (prop, values)| {
        let value = state.properties.iter().find(|(p, _)| p == prop).map(|(_, v)| v);
        let i = value.and_then(|v| values.iter().position(|x| x == v));
        let i = i.with_context(|| format!("{}: {prop}={value:?} not in registry", state.name))?;
        Ok(acc * values.len() as u32 + i as u32)
    })
}

fn from_ordinal(name: &str, info: &BlockInfo, mut ord: u32) -> Result<BlockState> {
    let mut props = Vec::with_capacity(info.properties.len());
    for (prop, values) in info.properties.iter().rev() {
        let n = values.len() as u32;
        anyhow::ensure!(n > 0, "corrupt pack: {name}: property {prop} has no values");
        props.push((prop.clone(), values[(ord % n) as usize].clone()));
        ord /= n;
    }
    anyhow::ensure!(ord == 0, "corrupt pack: {name}: state ordinal out of range");
    Ok(BlockState::new(name.to_owned(), props))
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
#[path = "format_tests.rs"]
mod tests;
