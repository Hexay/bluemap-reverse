//! A pack bundles everything reconstruction needs for one BlueMap + Minecraft version, so users need no
//! Java, server or BlueMap: the learned signature library, the block registry, the void template world,
//! and the texture list it was learned from (compatibility baseline + version fingerprint).

mod compat;
mod format;
mod index;
mod select;

use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail, ensure};
use bmr_fetch::LocalMap;
use bmr_invert::Library;
use bmr_invert::tints::BiomeTint;
use bmr_world::{BlockRegistry, PaletteStyle, TemplateFiles, World};
use lzma_rust2::{XzOptions, XzReader, XzWriter};

pub use compat::{Compat, Verdict, check};
pub use format::{Header, Meta};
use format::{Body, FORMAT, MAGIC, decode, encode};
pub use index::{Index, IndexEntry};
pub use select::{Candidate, Ranked, Source, indexed, installed, rank};

pub struct Pack {
    pub meta: Meta,
    pub library: Library,
    pub registry: Arc<BlockRegistry>,
    pub template: TemplateFiles,
    /// Texture names of the map the library was learned from.
    pub site_textures: Vec<String>,
    /// Grass/foliage/water tint BlueMap draws per overworld biome, commonest first (empty: biomes unknown).
    pub biome_tints: Vec<BiomeTint>,
}

impl Pack {
    /// From the local debug-world mirror + world, registry and template world (maintainer side), and the
    /// `biomes` fixture's mirror + world for the biome tint table.
    pub fn build(
        lib_map: &LocalMap,
        lib_world: &World,
        registry: Arc<BlockRegistry>,
        template_dir: &Path,
        mc_version: &str,
        biomes: Option<(&LocalMap, &World)>,
    ) -> Result<Self> {
        let biome_tints = biomes.map(|(map, world)| bmr_invert::tints::learn(map, world)).transpose()?.unwrap_or_default();
        let library = Library::build(lib_map, lib_world, &registry)?;
        let bluemap_version = lib_map.bluemap_version.clone().context("library mirror has no BlueMap version")?;
        let style = lib_world.palette_style()?.context("debug world has no palettes to learn the format from")?;
        Ok(Self {
            meta: Meta {
                mc_version: mc_version.to_owned(),
                data_version: library.data_version,
                bluemap_version,
                compact_palette: style == PaletteStyle::Compact,
                created_unix: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64,
            },
            site_textures: bmr_prbm::parse_texture_names(&lib_map.textures_json()?)?,
            library,
            registry,
            template: bmr_world::read_template(template_dir)?,
            biome_tints,
        })
    }

    pub fn palette_style(&self) -> PaletteStyle {
        if self.meta.compact_palette { PaletteStyle::Compact } else { PaletteStyle::Legacy }
    }

    /// Returns the file size in bytes.
    pub fn save(&self, path: &Path) -> Result<u64> {
        let header = Header { format: FORMAT, meta: self.meta.clone(), site_textures: self.site_textures.clone() };
        let mut registry: Vec<_> = self.registry.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        registry.sort_by(|a, b| a.0.cmp(&b.0));
        let mut body = encode(&self.library.entries, registry, self.template.clone())?;
        body.biome_tints = self.biome_tints.iter().map(|b| (b.biome.clone(), b.tints)).collect();
        let header_bytes = compress(&postcard::to_stdvec(&header)?)?;
        let mut bytes = Vec::from(&MAGIC[..]);
        bytes.extend((header_bytes.len() as u32).to_le_bytes());
        bytes.extend(header_bytes);
        bytes.extend(compress(&postcard::to_stdvec(&body)?)?);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, &bytes).with_context(|| path.display().to_string())?;
        Ok(bytes.len() as u64)
    }

    /// The saved file at `path` loads back to this pack's library (the encoding is lossless).
    pub fn verify_saved(&self, path: &Path) -> Result<()> {
        let back = Self::load(path)?;
        let (a, b) = (&self.library.entries, &back.library.entries);
        ensure!(a.len() == b.len(), "{}: {} entries saved, {} loaded", path.display(), a.len(), b.len());
        if let Some(i) = (0..a.len()).find(|&i| a[i] != b[i]) {
            bail!("{}: entry {i} ({:?}) differs after loading", path.display(), a[i].state);
        }
        Ok(())
    }

    /// Header only: versions and texture fingerprint, without decompressing the body.
    pub fn read_header(path: &Path) -> Result<Header> {
        let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
        Ok(split(&bytes, path)?.0)
    }

    /// Bytes per part: the header and compressed body as stored, then each body field uncompressed.
    pub fn sizes(path: &Path) -> Result<Vec<(&'static str, usize)>> {
        let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
        let (_, compressed) = split(&bytes, path)?;
        let body = read_body(compressed)?;
        let len = |v: Result<Vec<u8>, postcard::Error>| v.map(|b| b.len());
        Ok(vec![
            ("header", bytes.len() - compressed.len()),
            ("body (xz)", compressed.len()),
            ("  key textures (raw)", len(postcard::to_stdvec(&body.key_textures))?),
            ("  face keys (raw)", len(postcard::to_stdvec(&body.keys))?),
            ("  signatures (raw)", len(postcard::to_stdvec(&body.sigs))?),
            ("  uv sets (raw)", len(postcard::to_stdvec(&body.uv_sets))?),
            ("  entries (raw)", len(postcard::to_stdvec(&body.entries))?),
            ("  registry (raw)", len(postcard::to_stdvec(&body.registry))?),
            ("  template (raw)", len(postcard::to_stdvec(&body.template))?),
        ])
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
        let (header, compressed) = split(&bytes, path)?;
        let mut body = read_body(compressed)?;
        let biome_tints = std::mem::take(&mut body.biome_tints).into_iter().map(|(biome, tints)| BiomeTint { biome, tints }).collect();
        let (entries, registry, template) = decode(body);
        Ok(Self {
            library: Library::from_entries(entries, header.meta.data_version),
            meta: header.meta,
            registry: Arc::new(BlockRegistry::from_blocks(registry)),
            template,
            site_textures: header.site_textures,
            biome_tints,
        })
    }
}

fn read_body(compressed: &[u8]) -> Result<Body> {
    postcard::from_bytes(&decompress(compressed)?).context("corrupt pack body")
}

fn compress(raw: &[u8]) -> Result<Vec<u8>> {
    let mut opts = XzOptions::with_preset(9);
    // the reader allocates the whole declared dictionary (64 MB at preset 9)
    opts.lzma_options.dict_size = (raw.len() as u32).next_power_of_two().clamp(1 << 12, opts.lzma_options.dict_size);
    let mut xz = XzWriter::new(Vec::new(), opts)?;
    xz.write_all(raw)?;
    Ok(xz.finish()?)
}

fn decompress(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    XzReader::new(compressed, false).read_to_end(&mut raw)?;
    Ok(raw)
}

/// (header, compressed body) after validating magic and format.
fn split<'a>(bytes: &'a [u8], path: &Path) -> Result<(Header, &'a [u8])> {
    ensure!(bytes.len() > 12 && bytes.starts_with(MAGIC), "{} is not a bmr pack", path.display());
    let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    ensure!(bytes.len() >= 12 + len, "{}: truncated header", path.display());
    let header: Header = decompress(&bytes[12..12 + len])
        .ok()
        .and_then(|raw| postcard::from_bytes(&raw).ok())
        .with_context(|| format!("{}: unreadable header (older pack format?)", path.display()))?;
    if header.format != FORMAT {
        bail!("{}: pack format {} not supported by this bmr (expects {FORMAT})", path.display(), header.format);
    }
    Ok((header, &bytes[12 + len..]))
}
