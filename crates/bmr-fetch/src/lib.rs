//! Scrape a BlueMap webroot into a local mirror (settings, textures.json, lowres PNGs, hires PRBM).

mod discover;
pub mod grid;
mod http;
mod local;
pub mod lowres;
mod mirror;
mod progress;
pub mod settings;
pub mod store;

pub use http::Http;
pub use local::LocalMap;
pub use mirror::{MapSummary, Options, mirror};
pub use progress::{OnProgress, Progress};
