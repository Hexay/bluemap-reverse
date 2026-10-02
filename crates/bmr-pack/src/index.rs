//! Pack index: `index.json` next to downloadable packs, listing each pack's versions, hash and texture
//! fingerprint (a bitset over one shared name table, so the index stays small as packs are added).
//! `pull` ranks indexed packs alongside installed ones and downloads only the one it picks.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::Pack;
use crate::format::FORMAT;

const INDEX_FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct Index {
    pub format: u32,
    /// Union of all packs' texture names.
    pub textures: Vec<String>,
    pub packs: Vec<IndexEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// Relative to the index URL.
    pub file: String,
    pub size: u64,
    pub sha256: String,
    pub pack_format: u32,
    pub mc_version: String,
    pub bluemap_version: String,
    /// Base64 bitset over `Index::textures`.
    textures: String,
}

impl Index {
    pub fn build(packs: &[PathBuf]) -> Result<Self> {
        let mut loaded = Vec::new();
        for path in packs {
            let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
            loaded.push((path, Pack::read_header(path)?, bytes));
        }
        let mut textures: Vec<String> = loaded.iter().flat_map(|(_, h, _)| h.site_textures.iter().cloned()).collect();
        textures.sort();
        textures.dedup();
        let packs = loaded
            .into_iter()
            .map(|(path, header, bytes)| {
                let mut bits = vec![0u8; textures.len().div_ceil(8)];
                for t in &header.site_textures {
                    let i = textures.binary_search(t).expect("union contains every name");
                    bits[i / 8] |= 1 << (i % 8);
                }
                IndexEntry {
                    file: path.file_name().expect("pack path has a file name").to_string_lossy().into_owned(),
                    size: bytes.len() as u64,
                    sha256: sha256_hex(&bytes),
                    pack_format: header.format,
                    mc_version: header.meta.mc_version,
                    bluemap_version: header.meta.bluemap_version,
                    textures: B64.encode(bits),
                }
            })
            .collect();
        Ok(Self { format: INDEX_FORMAT, textures, packs })
    }

    pub fn parse(json: &[u8]) -> Result<Self> {
        let index: Self = serde_json::from_slice(json).context("unreadable pack index")?;
        ensure!(
            index.format == INDEX_FORMAT,
            "pack index format {} not supported (expects {INDEX_FORMAT})",
            index.format
        );
        Ok(index)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, serde_json::to_vec_pretty(self)?).with_context(|| path.display().to_string())
    }

    /// Entries this bmr can read.
    pub fn usable(&self) -> impl Iterator<Item = &IndexEntry> {
        self.packs.iter().filter(|e| e.pack_format == FORMAT)
    }

    pub fn textures_of(&self, e: &IndexEntry) -> Result<Vec<String>> {
        let bits = B64.decode(&e.textures).with_context(|| format!("{}: bad texture bitset", e.file))?;
        Ok(self
            .textures
            .iter()
            .enumerate()
            .filter(|(i, _)| bits.get(i / 8).is_some_and(|b| b & (1 << (i % 8)) != 0))
            .map(|(_, t)| t.clone())
            .collect())
    }
}

impl IndexEntry {
    /// Downloaded bytes are the file the index describes.
    pub fn verify(&self, bytes: &[u8]) -> Result<()> {
        ensure!(bytes.len() as u64 == self.size, "{}: {} bytes, index says {}", self.file, bytes.len(), self.size);
        ensure!(sha256_hex(bytes) == self.sha256, "{}: sha256 mismatch", self.file);
        Ok(())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}
