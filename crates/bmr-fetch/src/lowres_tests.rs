use super::*;

#[test]
fn huge_declared_png_is_refused_before_allocating() {
    // fuzzer find (texture_png target, IHDR CRC fixed): 71 bytes declaring 16×218M pixels, a 3.4 GB buffer
    let png = [
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 16, 13, 0, 0, 0, 1, 0, 0, 0, 0, 197, 17,
        39, 0, 0, 37, 0, 14, 73, 68, 65, 84, 120, 94, 99, 248, 255, 159, 129, 20, 4, 0, 253, 78, 31, 225, 190, 27, 58,
        22, 0, 40, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let e = LowresImage::decode(&png).err().expect("expected an error");
    assert!(e.to_string().contains("png too large"), "{e:#}");
}

/// A `w`×`2h` RGBA tile: colour half opaque, height half encodes y = 64 + x + z.
fn tile(w: u32, h: u32) -> Vec<u8> {
    let mut rgba = Vec::new();
    for row in 0..2 * h {
        for x in 0..w {
            if row < h {
                rgba.extend([10, 20, 30, 255]);
            } else {
                let [g, b] = (64 + (x + row - h) as i16).to_be_bytes();
                rgba.extend([0, g, b, 255]);
            }
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, 2 * h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    out
}

#[test]
fn lookups_inside_the_tile_decode_colour_and_height() {
    let img = LowresImage::decode(&tile(3, 2)).unwrap();
    assert_eq!((img.width, img.height), (3, 2));
    assert_eq!(img.color(2, 1), Some([10, 20, 30, 255]));
    assert_eq!(img.block_height(2, 1), Some(67));
    assert_eq!(img.visible_pixels().len(), 6);
}

#[test]
fn lookups_outside_the_tile_are_none_not_a_panic() {
    let img = LowresImage::decode(&tile(3, 2)).unwrap();
    // x past the width must not wrap into the next row; z past the colour half must not read heights
    assert_eq!(img.color(3, 0), None);
    assert_eq!(img.color(0, 2), None);
    assert_eq!(img.block_height(3, 0), None);
    assert_eq!(img.block_height(0, 2), None);
    assert_eq!(img.color(usize::MAX, usize::MAX), None);
}
