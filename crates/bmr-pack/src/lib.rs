//! A pack bundles everything reconstruction needs for one BlueMap + Minecraft version, so users need no
//! Java, server or BlueMap: the learned signature library, the block registry, the void template world,
//! and the texture list it was learned from (compatibility baseline).

mod compat;
mod format;

use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, bail, ensure};
use bmr_fetch::LocalMap;
use bmr_invert::Library;
use bmr_invert::face::Tex;
use bmr_world::{BlockRegistry, TemplateFiles, World};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

pub use compat::{Compat, Verdict, check};
pub use format::Meta;
use format::{FORMAT, MAGIC, PackFile, TexTable, dto_to_entry, entry_to_dto};

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
        Ok(Self {
            meta: Meta {
                mc_version: mc_version.to_owned(),
                data_version: library.data_version,
                bluemap_version,
                created_unix: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64,
            },
            site_textures: bmr_prbm::parse_texture_names(&lib_map.textures_json()?)?,
            library,
            registry,
            template: bmr_world::read_template(template_dir)?,
        })
    }

    /// Returns the file size in bytes.
    pub fn save(&self, path: &Path) -> Result<u64> {
        let mut tex = TexTable::default();
        let entries = self.library.entries.iter().map(|e| entry_to_dto(e, &mut tex)).collect();
        let file = PackFile {
            format: FORMAT,
            meta: self.meta.clone(),
            site_textures: self.site_textures.clone(),
            key_textures: tex.names,
            entries,
            registry: self.registry.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            template: self.template.clone(),
        };
        let mut gz = GzEncoder::new(Vec::from(&MAGIC[..]), Compression::best());
        gz.write_all(&postcard::to_stdvec(&file)?)?;
        let bytes = gz.finish()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, &bytes).with_context(|| path.display().to_string())?;
        Ok(bytes.len() as u64)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
        ensure!(bytes.starts_with(MAGIC), "{} is not a bmr pack", path.display());
        let mut raw = Vec::new();
        GzDecoder::new(&bytes[MAGIC.len()..]).read_to_end(&mut raw)?;
        let file: PackFile = postcard::from_bytes(&raw).context("corrupt pack")?;
        if file.format != FORMAT {
            bail!("pack format {} not supported by this bmr (expects {FORMAT}); get a matching pack", file.format);
        }
        let textures: Vec<Tex> = file.key_textures.iter().map(|n| Tex::intern(n)).collect();
        let entries = file.entries.into_iter().map(|d| dto_to_entry(d, &textures)).collect();
        Ok(Self {
            library: Library::from_entries(entries, file.meta.data_version),
            meta: file.meta,
            registry: Arc::new(BlockRegistry::from_blocks(file.registry)),
            template: file.template,
            site_textures: file.site_textures,
        })
    }
}
