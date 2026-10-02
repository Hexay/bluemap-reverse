//! Chunk → WorldWriter → region files → World → Chunk, in both palette encodings.

use std::path::PathBuf;
use std::sync::Arc;

use bmr_world::{BlockInfo, BlockRegistry, BlockState, Chunk, PaletteStyle, World, WorldWriter};

const DATA_VERSION: i32 = 4800;
const STAIRS_DEFAULT: [(&str, &str); 4] =
    [("facing", "north"), ("half", "bottom"), ("shape", "straight"), ("waterlogged", "false")];

/// Fresh empty directory under cargo's per-target tmp dir.
fn temp_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn state(name: &str, props: &[(&str, &str)]) -> BlockState {
    BlockState::new(format!("minecraft:{name}"), props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}

/// Every block the tests use; `filler_<n>` blocks pad a palette past 16 entries.
fn registry() -> BlockRegistry {
    let info = |default: &[(&str, &str)]| BlockInfo {
        properties: default.iter().map(|(k, v)| (k.to_string(), vec![v.to_string()])).collect(),
        default: state("x", default).properties,
    };
    let mut blocks = vec![
        ("minecraft:air".to_owned(), info(&[])),
        ("minecraft:stone".to_owned(), info(&[])),
        ("minecraft:oak_stairs".to_owned(), info(&STAIRS_DEFAULT)),
        ("minecraft:chest".to_owned(), info(&[("facing", "north"), ("type", "single"), ("waterlogged", "false")])),
    ];
    blocks.extend((0..20).map(|i| (format!("minecraft:filler_{i}"), info(&[]))));
    BlockRegistry::from_blocks(blocks)
}

/// Section -1 uniform stone, 0 mixed (properties, defaults and non-defaults, 24-entry palette → 5 bits,
/// two biomes), 1 left all air.
fn sample_chunk(x: i32, z: i32) -> Chunk {
    let mut c = Chunk::new(x, z, DATA_VERSION, (-1, 1), "minecraft:plains");
    c.sections[0].palette = vec![state("stone", &[])];
    let s = &mut c.sections[1];
    s.palette = vec![
        state("air", &[]),
        state("oak_stairs", &STAIRS_DEFAULT),
        state("oak_stairs", &[("facing", "south"), ("half", "top"), ("shape", "outer_left"), ("waterlogged", "true")]),
        state("chest", &[("facing", "east"), ("type", "left"), ("waterlogged", "false")]),
    ];
    s.palette.extend((0..20).map(|i| state(&format!("filler_{i}"), &[])));
    let n = s.palette.len();
    s.blocks = (0..4096).map(|i| ((i * 7 + x.unsigned_abs() as usize) % n) as u16).collect();
    s.set_biome(1, 2, 3, "minecraft:desert");
    s.set_biome(0, 0, 0, "minecraft:desert");
    c
}

fn assert_same(got: &Chunk, want: &Chunk) {
    let pos = (want.x, want.z);
    assert_eq!((got.x, got.z, got.data_version, got.status.as_str()), (want.x, want.z, DATA_VERSION, "minecraft:full"));
    assert_eq!(got.sections.len(), want.sections.len(), "{pos:?}");
    for (g, w) in got.sections.iter().zip(&want.sections) {
        assert_eq!(g.y, w.y, "{pos:?}");
        assert_eq!(g.palette, w.palette, "{pos:?} section {}", w.y);
        assert!((0..4096).all(|i| g.index(i) == w.index(i)), "{pos:?} section {} blocks differ", w.y);
        assert_eq!(g.biome_palette, w.biome_palette, "{pos:?} section {}", w.y);
        assert_eq!(g.biomes, w.biomes, "{pos:?} section {}", w.y);
    }
}

fn roundtrip(style: PaletteStyle, name: &str) {
    let out = temp_dir(name);
    let registry = Arc::new(registry());
    let template = vec![("level.dat".to_owned(), b"stub".to_vec()), ("data/raids.dat".to_owned(), Vec::new())];
    let writer = WorldWriter::create(&out, &template, "minecraft:overworld", registry.clone(), style).expect("create");
    let positions = [(0, 0), (31, 31), (-1, 33), (40, 0)];
    let regions = writer.write_chunks(positions.iter().map(|&(x, z)| sample_chunk(x, z)).collect()).unwrap();
    assert_eq!(regions, 3);
    assert_eq!(std::fs::read(out.join("data/raids.dat")).unwrap(), Vec::<u8>::new());

    let world = World::open(&out, "minecraft:overworld", Some(registry)).unwrap();
    assert_eq!(world.regions().unwrap(), vec![(-1, 1), (0, 0), (1, 0)]);
    assert_eq!(world.palette_style().unwrap(), Some(style));
    for (x, z) in positions {
        let region = world.read_region((x.div_euclid(32), z.div_euclid(32))).unwrap();
        let got = region.get(&(x, z)).unwrap_or_else(|| panic!("chunk {x},{z} missing"));
        assert_eq!(got.palette_style, Some(style));
        assert_same(got, &sample_chunk(x, z));
    }
    let r00 = world.read_region_where((0, 0), &|p| p == (31, 31)).unwrap();
    assert_eq!(r00.keys().copied().collect::<Vec<_>>(), vec![(31, 31)]);
    assert!(world.read_region((7, 7)).unwrap().is_empty());
}

#[test]
fn legacy_palette_roundtrip() {
    roundtrip(PaletteStyle::Legacy, "roundtrip-legacy");
}

#[test]
fn compact_palette_roundtrip() {
    roundtrip(PaletteStyle::Compact, "roundtrip-compact");
}

#[test]
fn compact_palette_needs_a_registry_to_read() {
    let out = temp_dir("compact-no-registry");
    let template = vec![("level.dat".to_owned(), Vec::new())];
    let writer =
        WorldWriter::create(&out, &template, "minecraft:overworld", Arc::new(registry()), PaletteStyle::Compact)
            .unwrap();
    writer.write_chunks(vec![sample_chunk(0, 0)]).unwrap();
    let e =
        World::open(&out, "minecraft:overworld", None).unwrap().read_region((0, 0)).err().expect("read should fail");
    assert!(format!("{e:#}").contains("block registry is required"), "{e:#}");
}

#[test]
fn compact_writer_rejects_unknown_blocks() {
    let out = temp_dir("compact-unknown");
    let template = vec![("level.dat".to_owned(), Vec::new())];
    let writer =
        WorldWriter::create(&out, &template, "minecraft:overworld", Arc::new(registry()), PaletteStyle::Compact)
            .unwrap();
    let mut c = sample_chunk(0, 0);
    c.sections[0].palette = vec![state("not_a_block", &[])];
    let e = writer.write_chunks(vec![c]).unwrap_err();
    assert!(format!("{e:#}").contains("unknown block minecraft:not_a_block"), "{e:#}");
}

#[test]
fn modern_template_layout_and_overwrite_guards() {
    let out = temp_dir("modern-layout");
    let template =
        vec![("level.dat".to_owned(), Vec::new()), ("dimensions/minecraft/overworld/data/x".to_owned(), Vec::new())];
    let new =
        |dim: &str| WorldWriter::extend(&out, &template, dim, Arc::new(registry()), PaletteStyle::Legacy).map(|_| ());
    assert!(new("minecraft:the_nether").is_err(), "extend needs an existing world");
    let writer =
        WorldWriter::create(&out, &template, "minecraft:overworld", Arc::new(registry()), PaletteStyle::Legacy)
            .unwrap();
    writer.write_chunks(vec![sample_chunk(0, 0)]).unwrap();
    assert!(out.join("dimensions/minecraft/overworld/region/r.0.0.mca").is_file());
    assert!(
        WorldWriter::create(&out, &template, "minecraft:overworld", Arc::new(registry()), PaletteStyle::Legacy)
            .is_err()
    );
    assert!(new("minecraft:overworld").is_err(), "overworld already has chunks");
    new("minecraft:the_nether").unwrap();
}
