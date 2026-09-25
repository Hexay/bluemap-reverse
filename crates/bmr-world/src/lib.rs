//! Anvil world IO: region container, chunk NBT → block/biome palettes. Read side (write side: phase 4).

mod chunk;
mod nbt;
mod region;
mod registry;
mod world;

pub use chunk::{BlockState, Chunk, Section, is_air_name};
pub use registry::{BlockInfo, BlockRegistry};
pub use world::{ChunkPos, World};
