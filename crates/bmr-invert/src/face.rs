//! Quads → (block cell, position-independent key). Keys leave UVs out: BlueMap picks rotated/mirrored
//! model variants by position hash, which changes UVs but not geometry or texture. UVs are kept beside
//! the keys (`Uv`) only to choose among states with identical keys (door hinge, texture rotation).
//! Keys are per quad, not per triangle: a rotated variant moves the quad's diagonal.

mod key;
mod world;

pub use key::*;
pub use world::*;

pub use crate::texture::Tex;

#[cfg(test)]
#[path = "face_tests.rs"]
mod tests;
