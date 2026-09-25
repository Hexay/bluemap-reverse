//! Scrape a BlueMap webroot into a local mirror (settings, textures.json, lowres PNGs, hires PRBM).

mod discover;
pub mod grid;
mod http;
pub mod lowres;
mod mirror;
pub mod settings;
pub mod store;

pub use http::decompress;
pub use mirror::{MapSummary, Options, mirror};
