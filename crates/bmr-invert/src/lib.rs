//! Invert BlueMap hires tiles into block states using a signature library learned from the debug world.

pub mod face;
pub mod library;
pub mod matcher;
pub mod reverse;

pub use library::Library;
pub use reverse::{Inverted, Stats, reverse};
