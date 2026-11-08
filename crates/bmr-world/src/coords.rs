//! Block, section, chunk and region coordinates. All of them floor, so block -1 is in chunk -1.

/// Chunk holding block column (x, z).
pub fn chunk_of(x: i32, z: i32) -> (i32, i32) {
    (x.div_euclid(16), z.div_euclid(16))
}

/// Region holding block column (x, z).
pub fn region_of(x: i32, z: i32) -> (i32, i32) {
    (x.div_euclid(512), z.div_euclid(512))
}

/// Region holding chunk (cx, cz).
pub fn region_of_chunk(cx: i32, cz: i32) -> (i32, i32) {
    (cx.div_euclid(32), cz.div_euclid(32))
}

/// Section holding block height `y`.
pub fn section_of(y: i32) -> i32 {
    y.div_euclid(16)
}

/// A block coordinate's offset (0..16) inside its chunk or section.
pub fn local(v: i32) -> usize {
    v.rem_euclid(16) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_coordinates_floor() {
        assert_eq!(chunk_of(-1, 16), (-1, 1));
        assert_eq!(region_of(-1, 512), (-1, 1));
        assert_eq!(region_of_chunk(-1, 32), (-1, 1));
        assert_eq!((section_of(-64), local(-1)), (-4, 15));
    }
}
