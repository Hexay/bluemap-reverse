//! On-disk pack layout: `MAGIC` + gzip(postcard(`PackFile`)). Texture ids in face keys are process-local,
//! so keys store an index into the pack's own texture-name table.

use bmr_invert::face::{FaceKey, Liquid, Tex};
use bmr_invert::library::Entry;
use bmr_world::{BlockInfo, BlockState};
use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8; 8] = b"BMRPACK\0";
/// Bump on any change to the structs below.
pub const FORMAT: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub mc_version: String,
    pub data_version: i32,
    pub bluemap_version: String,
    pub created_unix: i64,
}

#[derive(Serialize, Deserialize)]
pub struct PackFile {
    pub format: u32,
    pub meta: Meta,
    /// Every texture of the library map (index = material index there): the compatibility baseline.
    pub site_textures: Vec<String>,
    /// Texture names referenced by `entries` keys.
    pub key_textures: Vec<String>,
    pub entries: Vec<EntryDto>,
    pub registry: Vec<(String, BlockInfo)>,
    pub template: Vec<(String, Vec<u8>)>,
}

#[derive(Serialize, Deserialize)]
pub struct EntryDto {
    name: String,
    props: Vec<(String, String)>,
    sig: Vec<KeyDto>,
    overhang: Vec<([i32; 3], KeyDto)>,
    /// 0 none, 1 water, 2 lava
    liquid: u8,
    tint: Option<[u8; 3]>,
    default_distance: u32,
    full_cube: bool,
}

#[derive(Serialize, Deserialize)]
struct KeyDto {
    tex: u32,
    tinted: bool,
    verts: [[i16; 3]; 4],
}

/// Collects texture names while converting keys.
#[derive(Default)]
pub struct TexTable {
    pub names: Vec<String>,
    index: std::collections::HashMap<Tex, u32>,
}

impl TexTable {
    fn id(&mut self, t: Tex) -> u32 {
        *self.index.entry(t).or_insert_with(|| {
            self.names.push(t.name().to_string());
            (self.names.len() - 1) as u32
        })
    }

    fn key(&mut self, k: &FaceKey) -> KeyDto {
        KeyDto { tex: self.id(k.texture), tinted: k.tinted, verts: k.verts }
    }
}

pub fn entry_to_dto(e: &Entry, tex: &mut TexTable) -> EntryDto {
    EntryDto {
        name: e.state.name.clone(),
        props: e.state.properties.clone(),
        sig: e.sig.iter().map(|k| tex.key(k)).collect(),
        overhang: e.overhang.iter().map(|(o, k)| ([o.0, o.1, o.2], tex.key(k))).collect(),
        liquid: match e.liquid {
            None => 0,
            Some(Liquid::Water) => 1,
            Some(Liquid::Lava) => 2,
        },
        tint: e.tint,
        default_distance: e.default_distance,
        full_cube: e.full_cube,
    }
}

/// `textures` = the pack's `key_textures` interned in this process.
pub fn dto_to_entry(d: EntryDto, textures: &[Tex]) -> Entry {
    let key = |k: KeyDto| FaceKey { texture: textures[k.tex as usize], tinted: k.tinted, verts: k.verts };
    Entry {
        state: BlockState::new(d.name, d.props),
        sig: d.sig.into_iter().map(key).collect(),
        overhang: d.overhang.into_iter().map(|(o, k)| ((o[0], o[1], o[2]), key(k))).collect(),
        liquid: match d.liquid {
            1 => Some(Liquid::Water),
            2 => Some(Liquid::Lava),
            _ => None,
        },
        tint: d.tint,
        default_distance: d.default_distance,
        full_cube: d.full_cube,
    }
}
