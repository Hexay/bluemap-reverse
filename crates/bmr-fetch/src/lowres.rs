//! Lowres tile PNGs: top half = column colours, bottom half = height/blocklight (research/01 §5).

use std::io::Cursor;

use anyhow::{Context, Result, bail};

/// Pixel coords (x, z) in the colour half with alpha > 0, i.e. columns that have rendered geometry.
pub fn visible_pixels(png_bytes: &[u8]) -> Result<Vec<(i32, i32)>> {
    let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().context("png too large")?];
    let info = reader.next_frame(&mut buf)?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        bail!("expected 8-bit RGBA lowres tile, got {:?}/{:?}", info.color_type, info.bit_depth);
    }
    let (w, h) = (info.width as usize, info.height as usize);
    let mut out = Vec::new();
    for z in 0..h / 2 {
        let row = &buf[z * info.line_size..][..w * 4];
        for x in 0..w {
            if row[x * 4 + 3] > 0 {
                out.push((x as i32, z as i32));
            }
        }
    }
    Ok(out)
}
