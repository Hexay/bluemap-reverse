use std::fmt;

/// Block state with properties sorted by key; `Display` gives the canonical
/// `minecraft:oak_stairs[facing=south,half=bottom]` form used for comparison and reports.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockState {
    pub name: String,
    pub properties: Vec<(String, String)>,
}

impl BlockState {
    pub fn new(name: String, mut properties: Vec<(String, String)>) -> Self {
        properties.sort();
        Self { name, properties }
    }

    pub fn is_air(&self) -> bool {
        is_air_name(&self.name)
    }
}

pub fn is_air_name(name: &str) -> bool {
    matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.name)?;
        if !self.properties.is_empty() {
            let props: Vec<String> = self.properties.iter().map(|(k, v)| format!("{k}={v}")).collect();
            write!(f, "[{}]", props.join(","))?;
        }
        Ok(())
    }
}

pub struct Section {
    /// Section index; blocks span y*16 .. y*16+15.
    pub y: i32,
    pub palette: Vec<BlockState>,
    /// 4096 palette indices, `(y*16 + z)*16 + x`.
    pub blocks: Vec<u16>,
    pub biome_palette: Vec<String>,
    /// 64 indices over 4×4×4 cells, `(y*4 + z)*4 + x`.
    pub biomes: Vec<u16>,
}

impl Section {
    pub fn block(&self, x: usize, y: usize, z: usize) -> &BlockState {
        &self.palette[self.blocks[(y * 16 + z) * 16 + x] as usize]
    }

    pub fn biome(&self, cx: usize, cy: usize, cz: usize) -> Option<&str> {
        self.biome_palette.get(self.biomes[(cy * 4 + cz) * 4 + cx] as usize).map(String::as_str)
    }
}

pub struct Chunk {
    pub x: i32,
    pub z: i32,
    pub data_version: i32,
    /// `minecraft:full` for completed chunks; worldgen leaves partial ones at the edges.
    pub status: String,
    /// Sorted by `y`; sections without block data are omitted.
    pub sections: Vec<Section>,
}

impl Chunk {
    pub fn is_full(&self) -> bool {
        self.status == "minecraft:full" || self.status == "full"
    }

    pub fn section(&self, sy: i32) -> Option<&Section> {
        self.sections.binary_search_by_key(&sy, |s| s.y).ok().map(|i| &self.sections[i])
    }

    /// World y, chunk-local x/z. `None` where no section exists (treat as air).
    pub fn block(&self, x: usize, y: i32, z: usize) -> Option<&BlockState> {
        self.section(y.div_euclid(16)).map(|s| s.block(x, y.rem_euclid(16) as usize, z))
    }

    pub fn y_range(&self) -> Option<(i32, i32)> {
        Some((self.sections.first()?.y * 16, self.sections.last()?.y * 16 + 15))
    }
}
