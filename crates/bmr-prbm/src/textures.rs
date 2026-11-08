//! `textures.json`: index = PRBM material index (docs/research/01 §2.4).

use std::io::Cursor;

use anyhow::{Context, Result, bail};
use base64::Engine;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
    /// e.g. `minecraft:block/stone`; index 0 is `bluemap:block/missing`.
    pub resource_path: String,
    /// Average straight RGBA.
    pub color: [f32; 4],
    pub half_transparent: bool,
    /// `data:image/png;base64,…`; animated textures are vertical frame strips.
    pub texture: String,
    #[serde(default)]
    pub animation: Option<serde_json::Value>,
}

pub fn parse_textures(json: &[u8]) -> Result<Vec<Texture>> {
    serde_json::from_slice(json).context("textures.json")
}

/// Resource paths only (index = material index): skips allocating the embedded PNGs.
pub fn parse_texture_names(json: &[u8]) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct NameOnly {
        resource_path: String,
    }
    let v: Vec<NameOnly> = serde_json::from_slice(json).context("textures.json")?;
    Ok(v.into_iter().map(|t| t.resource_path).collect())
}

impl Texture {
    pub fn png(&self) -> Result<Vec<u8>> {
        let Some(b64) = self.texture.strip_prefix("data:image/png;base64,") else {
            bail!("{}: not a base64 PNG data URI", self.resource_path);
        };
        Ok(base64::engine::general_purpose::STANDARD.decode(b64)?)
    }

    /// The PNG as PRBM UVs see it: animated strips cropped to their first (top, square) frame.
    pub fn frame_png(&self) -> Result<Vec<u8>> {
        let png = self.png()?;
        if self.animation.is_none() {
            return Ok(png);
        }
        crop_top_square(&png).with_context(|| self.resource_path.clone())
    }

    /// Filesystem-safe name, e.g. `minecraft_block_stone`.
    pub fn file_stem(&self) -> String {
        let safe = |c: char| c.is_ascii_alphanumeric() || "-_.".contains(c);
        self.resource_path.chars().map(|c| if safe(c) { c } else { '_' }).collect()
    }
}

/// Decoded size cap for embedded textures (vanilla's largest animated strips are well under 1 MB).
const MAX_PNG_BYTES: usize = 64 << 20;

fn crop_top_square(png_bytes: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info()?;
    let size = reader.output_buffer_size().filter(|&n| n <= MAX_PNG_BYTES).context("png too large")?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf)?;
    let side = info.width.min(info.height);
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, info.width, side);
    enc.set_color(info.color_type);
    enc.set_depth(info.bit_depth);
    enc.write_header()?.write_image_data(&buf[..info.line_size * side as usize])?;
    Ok(out)
}

#[cfg(test)]
#[path = "textures_tests.rs"]
mod tests;
