//! Two tiny worlds written with bmr-world's writer, scored end to end. Columns scored: x ∈ {-16, 0, 1}, z = 0;
//! one section (y 0..=15); chunks (0,0) and (-1,0) sit in different regions, so per-region reports merge.

use std::path::PathBuf;
use std::sync::Arc;

use bmr_score::{Report, Scope, score};
use bmr_world::{BlockRegistry, BlockState, Chunk, PaletteStyle, World, WorldWriter};
use rustc_hash::{FxHashMap, FxHashSet};

const PLAINS: &str = "minecraft:plains";

fn bs(s: &str) -> BlockState {
    let (name, props) = s.split_once('[').map_or((s, ""), |(n, p)| (n, p.trim_end_matches(']')));
    let props = props.split(',').filter(|p| !p.is_empty()).map(|p| p.split_once('=').unwrap());
    BlockState::new(name.into(), props.map(|(k, v)| (k.into(), v.into())).collect())
}

fn chunk(cx: i32, blocks: &[((usize, i32, usize), &str)]) -> Chunk {
    let mut c = Chunk::new(cx, 0, 4000, (0, 0), PLAINS);
    let s = &mut c.sections[0];
    for &((x, y, z), state) in blocks {
        let state = bs(state);
        let idx = s.palette.iter().position(|p| *p == state).unwrap_or_else(|| {
            s.palette.push(state);
            s.palette.len() - 1
        });
        s.blocks.resize(4096, 0);
        s.blocks[(y as usize * 16 + z) * 16 + x] = idx as u16;
    }
    c
}

fn write_world(name: &str, chunks: Vec<Chunk>) -> World {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("bmr-score-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    let registry = Arc::new(BlockRegistry::from_blocks([]));
    let w = WorldWriter::create(&root, &[], "minecraft:overworld", registry, PaletteStyle::Legacy).unwrap();
    w.write_chunks(chunks).unwrap();
    World::open(&root, "minecraft:overworld", None).unwrap()
}

fn original(name: &str) -> World {
    let mut partial = chunk(1, &[((0, 0, 0), "minecraft:stone")]);
    partial.status = "minecraft:noise".into();
    write_world(
        name,
        vec![
            chunk(
                0,
                &[
                    ((0, 0, 0), "minecraft:stone"),
                    ((0, 1, 0), "minecraft:oak_log[axis=y]"),
                    ((1, 0, 0), "minecraft:dirt"),
                ],
            ),
            chunk(-1, &[((0, 0, 0), "minecraft:stone")]),
            partial,
        ],
    )
}

fn reconstructed(name: &str) -> World {
    let mut c = chunk(
        0,
        &[
            ((0, 0, 0), "minecraft:stone"),
            ((0, 1, 0), "minecraft:oak_log[axis=x]"),
            ((1, 5, 0), "minecraft:sand"),
            ((0, 7, 0), "minecraft:cave_air"),
        ],
    );
    c.sections[0].set_biome(0, 1, 0, "minecraft:desert");
    write_world(name, vec![c, chunk(-1, &[((0, 0, 0), "minecraft:stone")])])
}

fn columns(x: i32, z: i32) -> bool {
    z == 0 && [-16, 0, 1].contains(&x)
}

fn confusions(list: &[bmr_score::Confusion]) -> Vec<(&str, &str, u64)> {
    list.iter().map(|c| (c.original.as_str(), c.reconstructed.as_str(), c.count)).collect()
}

fn acc(a: bmr_score::Accuracy) -> (u64, u64, u64) {
    (a.total, a.exact, a.name)
}

#[test]
fn scores_known_differences() {
    let rendered: FxHashSet<(i32, i32, i32)> = [(0, 1, 0), (1, 0, 0)].into_iter().collect();
    let lookalikes: FxHashMap<String, u32> =
        [("minecraft:oak_log[axis=y]".to_owned(), 1), ("minecraft:oak_log[axis=x]".to_owned(), 1)]
            .into_iter()
            .collect();
    let scope = Scope {
        columns: &columns,
        rendered: Some(&rendered),
        sample: Some(("minecraft:dirt", "minecraft:air")),
        lookalikes: Some(&lookalikes),
    };
    let r: Report = score(&original("known-orig"), &reconstructed("known-recon"), &scope, 10).unwrap();

    assert_eq!((r.chunks, r.partial_chunks_skipped, r.columns), (2, 1, 3));
    // 48 voxels; cave_air counts as air
    assert_eq!(acc(r.all), (48, 45, 46));
    assert_eq!(acc(r.occupied), (5, 2, 3));
    assert_eq!((r.solid.original, r.solid.reconstructed, r.solid.both), (4, 4, 3));
    assert!((r.solid_iou() - 3.0 / 5.0).abs() < 1e-9);
    assert_eq!((r.surface.total, r.surface.hits), (3, 1));
    assert_eq!(acc(r.exposed), (4, 2, 3));
    assert_eq!(acc(r.rendered), (2, 0, 1));
    assert_eq!(r.rendered_alike, 1, "log axis is a declared look-alike");
    assert_eq!((r.biomes.total, r.biomes.hits), (8, 7));
    assert_eq!(r.samples, [(1, 0, 0)]);

    assert_eq!(
        confusions(&r.confusions),
        [
            ("minecraft:air", "minecraft:sand", 1),
            ("minecraft:dirt", "minecraft:air", 1),
            ("minecraft:oak_log[axis=y]", "minecraft:oak_log[axis=x]", 1),
        ]
    );
    assert_eq!(confusions(&r.rendered_confusions), [("minecraft:dirt", "minecraft:air", 1)]);
    assert_eq!(confusions(&r.biome_confusions), [(PLAINS, "minecraft:desert", 1)]);

    let text = r.to_string();
    assert!(text.contains("chunks 2 (1 partial skipped), columns 3"), "{text}");
    assert!(text.contains("counting look-alikes"), "{text}");
}

#[test]
fn top_confusions_are_truncated() {
    let scope = Scope { columns: &columns, rendered: None, sample: None, lookalikes: None };
    let r = score(&original("top-orig"), &reconstructed("top-recon"), &scope, 1).unwrap();
    assert_eq!(confusions(&r.confusions), [("minecraft:air", "minecraft:sand", 1)]);
    assert_eq!(r.rendered.total, 0);
    assert!(r.samples.is_empty());
}

#[test]
fn empty_reconstruction_scores_zero_occupied() {
    let missing = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bmr-score-no-such-world");
    let scope = Scope { columns: &columns, rendered: None, sample: None, lookalikes: None };
    let r = score(&original("empty-orig"), &World::empty(missing), &scope, 10).unwrap();
    assert_eq!(acc(r.occupied), (4, 0, 0));
    assert_eq!(r.solid.reconstructed, 0);
    assert_eq!((r.biomes.total, r.biomes.hits), (8, 0));
    assert!(r.solid_iou() == 0.0);
}
