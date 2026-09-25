//! A pack bundles everything reconstruction needs for one BlueMap + Minecraft version, so users need no
//! Java, server or BlueMap: the learned signature library, the block registry, the void template world,
//! and the texture list it was learned from (compatibility baseline + version fingerprint).

mod compat;
mod format;
mod select;

use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail, ensure};
use bmr_fetch::LocalMap;
use bmr_invert::Library;
use bmr_invert::face::Tex;
use bmr_world::{BlockRegistry, PaletteStyle, TemplateFiles, World};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

pub use compat::{Compat, Verdict, check};
pub use format::{Header, Meta};
use format::{Body, FORMAT, MAGIC, TexTable, dto_to_entry, entry_to_dto};
pub use select::{Ranked, rank};

pub struct Pack {
    pub meta: Meta,
    pub library: Library,
    pub registry: Arc<BlockRegistry>,
    pub template: TemplateFiles,
    /// Texture names of the map the library was learned from.
    pub site_textures: Vec<String>,
}

impl Pack {
    /// From the local debug-world mirror + world, registry and template world (maintainer side).
    pub fn build(
        lib_map: &LocalMap,
        lib_world: &World,
        registry: Arc<BlockRegistry>,
        template_dir: &Path,
        mc_version: &str,
    ) -> Result<Self> {
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
        })
    }

    pub fn palette_style(&self) -> PaletteStyle {
        if self.meta.compact_palette { PaletteStyle::Compact } else { PaletteStyle::Legacy }
    }

    /// Returns the file size in bytes.
    pub fn save(&self, path: &Path) -> Result<u64> {
        let header = Header { format: FORMAT, meta: self.meta.clone(), site_textures: self.site_textures.clone() };
        let mut tex = TexTable::default();
        let entries = self.library.entries.iter().map(|e| entry_to_dto(e, &mut tex)).collect();
        let body = Body {
            key_textures: tex.names,
            entries,
            registry: self.registry.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            template: self.template.clone(),
        };
        let header_bytes = postcard::to_stdvec(&header)?;
        let mut out = Vec::from(&MAGIC[..]);
        out.extend((header_bytes.len() as u32).to_le_bytes());
        out.extend(header_bytes);
        let mut gz = GzEncoder::new(out, Compression::best());
        gz.write_all(&postcard::to_stdvec(&body)?)?;
        let bytes = gz.finish()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, &bytes).with_context(|| path.display().to_string())?;
        Ok(bytes.len() as u64)
    }

    /// Header only: versions and texture fingerprint, without decompressing the body.
    pub fn read_header(path: &Path) -> Result<Header> {
        let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
        Ok(split(&bytes, path)?.0)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
        let (header, compressed) = split(&bytes, path)?;
        let mut raw = Vec::new();
        GzDecoder::new(compressed).read_to_end(&mut raw)?;
        let body: Body = postcard::from_bytes(&raw).context("corrupt pack body")?;
        let textures: Vec<Tex> = body.key_textures.iter().map(|n| Tex::intern(n)).collect();
        let entries = body.entries.into_iter().map(|d| dto_to_entry(d, &textures)).collect();
        Ok(Self {
            library: Library::from_entries(entries, header.meta.data_version),
            meta: header.meta,
            registry: Arc::new(BlockRegistry::from_blocks(body.registry)),
            template: body.template,
            site_textures: header.site_textures,
        })
    }
}

/// (header, compressed body) after validating magic and format.
fn split<'a>(bytes: &'a [u8], path: &Path) -> Result<(Header, &'a [u8])> {
    ensure!(bytes.len() > 12 && bytes.starts_with(MAGIC), "{} is not a bmr pack", path.display());
    let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    ensure!(bytes.len() >= 12 + len, "{}: truncated header", path.display());
    let header: Header = postcard::from_bytes(&bytes[12..12 + len])
        .with_context(|| format!("{}: unreadable header (older pack format?)", path.display()))?;
    if header.format != FORMAT {
        bail!("{}: pack format {} not supported by this bmr (expects {FORMAT})", path.display(), header.format);
    }
    Ok((header, &bytes[12 + len..]))
}
