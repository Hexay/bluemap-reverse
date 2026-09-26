//! Invert BlueMap hires tiles into block states using a signature library learned from the debug world.

pub mod evidence;
pub mod face;
pub mod library;
pub mod lookalike;
pub mod matcher;
pub mod overhang;
pub mod reverse;
pub mod texture;
pub mod timings;
pub mod tints;

pub use library::Library;
pub use reverse::{Inverted, Stats, map_textures, rendered_cells, reverse};
