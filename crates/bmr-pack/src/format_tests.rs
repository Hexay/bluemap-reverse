use super::*;

fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

fn key(t: &str, v: i16) -> FaceKey {
    FaceKey { texture: Tex::intern(t), tinted: false, verts: [[0, 0, 0], [v, 0, 0], [v, v, 0], [0, v, 0]] }
}

fn entry(
    state: BlockState,
    sig: Vec<FaceKey>,
    liquid: Option<Liquid>,
    overhang: Vec<((i32, i32, i32), FaceKey)>,
    tint: Option<[u8; 3]>,
    default_distance: u32,
) -> Entry {
    let uvs = (0..sig.len()).map(|i| [[i as i16, 0], [0, 1], [1, 1], [1, 0]]).collect();
    Entry { state, sig, uvs, light: 3, liquid, overhang, tint, default_distance, full_cube: false }
}

fn sample() -> (Vec<Entry>, Vec<(String, BlockInfo)>) {
    let stairs_props: Vec<(String, Vec<String>)> =
        [("facing", ["north", "south"]), ("half", ["top", "bottom"]), ("waterlogged", ["true", "false"])]
            .iter()
            .map(|(p, vs)| (p.to_string(), vs.iter().map(|v| v.to_string()).collect()))
            .collect();
    let registry = vec![
        (
            "minecraft:oak_stairs".to_string(),
            BlockInfo {
                properties: stairs_props,
                default: owned(&[("facing", "north"), ("half", "bottom"), ("waterlogged", "false")]),
            },
        ),
        ("minecraft:stone".to_string(), BlockInfo { properties: vec![], default: vec![] }),
    ];
    let stairs = BlockState::new(
        "minecraft:oak_stairs".into(),
        owned(&[("facing", "south"), ("half", "top"), ("waterlogged", "true")]),
    );
    let stone = BlockState::new("minecraft:stone".into(), vec![]);
    let entries = vec![
        entry(stone.clone(), vec![key("block/stone", 64)], None, vec![], None, 0),
        entry(
            stairs,
            vec![key("block/oak_planks", 32), key("block/stone", 64)],
            Some(Liquid::Water),
            vec![((0, 1, 0), key("block/oak_planks", 16))],
            Some([1, 2, 3]),
            3,
        ),
        entry(stone, vec![key("block/stone", 64)], None, vec![], None, 0),
    ];
    (entries, registry)
}

#[test]
fn roundtrip_is_lossless() {
    let (entries, registry) = sample();
    let body = encode(&entries, registry, vec![]).unwrap();
    assert_eq!(body.sigs.len(), 2, "identical signatures are stored once");
    let (back, _, _) = decode(body).unwrap();
    assert_eq!(entries, back);
}

#[test]
fn out_of_range_indices_are_errors() {
    let corruptions: [fn(&mut Body); 6] = [
        |b| b.entries[0].block = 99,
        |b| b.entries[0].sig = 99,
        |b| b.entries[0].uvs = 99,
        |b| b.entries[1].state = 999,
        |b| b.sigs[0][0] = 99,
        |b| b.keys[0].tex = 99,
    ];
    for (i, corrupt) in corruptions.iter().enumerate() {
        let (entries, registry) = sample();
        let mut body = encode(&entries, registry, vec![]).unwrap();
        corrupt(&mut body);
        let err = decode(body).err().unwrap_or_else(|| panic!("corruption {i} decoded"));
        assert!(err.to_string().contains("corrupt pack"), "{err}");
    }
}
