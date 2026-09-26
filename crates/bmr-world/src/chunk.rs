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
    /// 4096 palette indices, `(y*16 + z)*16 + x`, or **empty = every cell is `palette[0]`** (uniform
    /// sections — all air above ground, all stone deep down — cost nothing). Read through `index`.
    pub blocks: Vec<u16>,
    pub biome_palette: Vec<String>,
    /// 64 indices over 4×4×4 cells, `(y*4 + z)*4 + x`.
    pub biomes: Vec<u16>,
}

impl Section {
    /// All air, one biome.
    pub fn empty(y: i32, biome: &str) -> Self {
        Self {
            y,
            palette: vec![BlockState::new("minecraft:air".into(), Vec::new())],
            blocks: Vec::new(),
            biome_palette: vec![biome.to_owned()],
            biomes: vec![0; 64],
        }
    }

    /// Palette index of cell `i` (`(y*16 + z)*16 + x`).
    pub fn index(&self, i: usize) -> u16 {
        if self.blocks.is_empty() { 0 } else { self.blocks[i] }
    }

    pub fn block(&self, x: usize, y: usize, z: usize) -> &BlockState {
        &self.palette[self.index((y * 16 + z) * 16 + x) as usize]
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, state: &BlockState) {
        let idx = palette_index(&mut self.palette, state);
        if self.blocks.is_empty() {
            if idx == 0 {
                return;
            }
            self.blocks = vec![0; 4096];
        }
        self.blocks[(y * 16 + z) * 16 + x] = idx;
    }

    pub fn set_biome(&mut self, cx: usize, cy: usize, cz: usize, biome: &str) {
        let idx = match self.biome_palette.iter().position(|b| b == biome) {
            Some(i) => i,
            None => {
                self.biome_palette.push(biome.to_owned());
                self.biome_palette.len() - 1
            }
        };
        self.biomes[(cy * 4 + cz) * 4 + cx] = idx as u16;
    }

    /// Drop palette entries no longer referenced (after overwrites), remapping indices; a section left
    /// with one entry becomes uniform.
    pub fn compact(&mut self) {
        if self.blocks.is_empty() {
            self.palette.truncate(1);
            return;
        }
        let mut used = vec![false; self.palette.len()];
        for &i in &self.blocks {
            used[i as usize] = true;
        }
        let mut remap = vec![0u16; self.palette.len()];
        let mut palette = Vec::new();
        for (i, state) in self.palette.drain(..).enumerate() {
            if used[i] {
                remap[i] = palette.len() as u16;
                palette.push(state);
            }
        }
        self.palette = palette;
        if self.palette.len() == 1 {
            self.blocks = Vec::new();
            return;
        }
        for i in &mut self.blocks {
            *i = remap[*i as usize];
        }
    }

    pub fn biome(&self, cx: usize, cy: usize, cz: usize) -> Option<&str> {
        self.biome_palette.get(self.biomes[(cy * 4 + cz) * 4 + cx] as usize).map(String::as_str)
    }
}

/// How block-state palettes are encoded on disk; the writer must match the target version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteStyle {
    /// `{Name, Properties}` for every entry (1.18 … 26.2).
    Legacy,
    /// 26.3: default states as bare names, others `{id, properties}`.
    Compact,
}

pub struct Chunk {
    pub x: i32,
    pub z: i32,
    pub data_version: i32,
    /// `minecraft:full` for completed chunks; worldgen leaves partial ones at the edges.
    pub status: String,
    /// Sorted by `y`; sections without block data are omitted.
    pub sections: Vec<Section>,
    /// Encoding seen when read from disk (`None` for built chunks or all-air palettes).
    pub palette_style: Option<PaletteStyle>,
}

fn palette_index(palette: &mut Vec<BlockState>, state: &BlockState) -> u16 {
    match palette.iter().position(|p| p == state) {
        Some(i) => i as u16,
        None => {
            palette.push(state.clone());
            (palette.len() - 1) as u16
        }
    }
}

impl Chunk {
    /// Full-status chunk with empty (air) sections `min_section..=max_section`.
    pub fn new(x: i32, z: i32, data_version: i32, (min_section, max_section): (i32, i32), biome: &str) -> Self {
        Self {
            x,
            z,
            data_version,
            status: "minecraft:full".into(),
            sections: (min_section..=max_section).map(|y| Section::empty(y, biome)).collect(),
            palette_style: None,
        }
    }

    /// World y, chunk-local x/z. Returns false if y is outside the chunk's sections.
    pub fn set_block(&mut self, x: usize, y: i32, z: usize, state: &BlockState) -> bool {
        let sy = y.div_euclid(16);
        match self.sections.binary_search_by_key(&sy, |s| s.y) {
            Ok(i) => {
                self.sections[i].set_block(x, y.rem_euclid(16) as usize, z, state);
                true
            }
            Err(_) => false,
        }
    }

    /// Copy biome data for every section both chunks have.
    pub fn copy_biomes_from(&mut self, src: &Chunk) {
        for s in &mut self.sections {
            if let Some(o) = src.section(s.y) {
                s.biome_palette.clone_from(&o.biome_palette);
                s.biomes.clone_from(&o.biomes);
            }
        }
    }

    /// Set each 4×4×4 biome cell to `biome_at(world cell coordinates)`, where it returns one.
    pub fn set_biomes<'a>(&mut self, biome_at: impl Fn((i32, i32, i32)) -> Option<&'a str>) {
        let (x0, z0) = (self.x * 4, self.z * 4);
        for s in &mut self.sections {
            for cy in 0..4 {
                for cz in 0..4 {
                    for cx in 0..4 {
                        if let Some(b) = biome_at((x0 + cx as i32, s.y * 4 + cy as i32, z0 + cz as i32)) {
                            s.set_biome(cx, cy, cz, b);
                        }
                    }
                }
            }
        }
    }

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
