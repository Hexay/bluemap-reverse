use super::*;
use crate::test_util::*;

const UP: Cell = (0, 1, 0);

#[test]
fn key_is_cell_local_quantized_and_sorted() {
    let cell = (10, 64, -3);
    let f = world_side(cell, UP, STONE, [255; 3], 0);
    let (k, uv) = f.key_uv_at(cell);
    assert_eq!(k.verts, [[0, 64, 0], [0, 64, 64], [64, 64, 0], [64, 64, 64]]);
    assert!(!k.tinted);
    // uvs follow their vertices through the sort: corner (q,·,q) had uv (1,1)
    assert_eq!(uv[3], [256, 256]);
    assert_eq!(f.key_at((10, 63, -3)).verts[0], [0, 128, 0], "keys are relative to the given cell");
}

#[test]
fn key_ignores_uv_but_not_tint() {
    let cell = (0, 0, 0);
    let a = world_side(cell, UP, STONE, [255; 3], 0);
    let mut b = world_side(cell, UP, STONE, [255; 3], 0);
    b.uv.rotate_left(1);
    assert_eq!(a.key_at(cell), b.key_at(cell));
    assert_ne!(a.key_uv_at(cell).1, b.key_uv_at(cell).1);
    b.color = [120, 200, 80];
    assert!(b.key_at(cell).tinted);
}

#[test]
fn sub_block_vertices_round_to_1_64() {
    let mut f = world_side((0, 0, 0), UP, STONE, [255; 3], 0);
    f.verts[0][1] = 1.0 - 0.4 / Q;
    f.verts[1][1] = 0.5;
    let k = f.key_at((0, 0, 0));
    assert!(k.verts.iter().any(|v| v[1] == 32));
    assert!(k.verts.iter().filter(|v| v[1] == 64).count() == 3);
}

#[test]
fn owner_is_the_cell_the_face_faces_out_of() {
    for cell in [(10, 64, -3), (-1, -64, -1), (0, 0, 0)] {
        for d in DIRS {
            assert_eq!(world_side(cell, d, STONE, [255; 3], 0).owner(), cell, "{cell:?} {d:?}");
        }
    }
}

#[test]
fn boundary_and_full_side_classification() {
    let full = side(STONE, (1, 0, 0));
    assert_eq!(full.boundary_dir(), Some((1, 0, 0)));
    assert!(full.full_side() && full.axis_aligned());

    let half = key(STONE, [[64, 0, 0], [64, 32, 0], [64, 32, 64], [64, 0, 64]]);
    assert_eq!(half.boundary_dir(), Some((1, 0, 0)));
    assert!(!half.full_side());

    let mid = horizontal(STONE, 32);
    assert!(mid.axis_aligned() && !mid.on_boundary());
    assert_eq!(horizontal(STONE, 0).boundary_dir(), Some((0, -1, 0)));

    let diagonal = cross(STONE, 0, 0)[0];
    assert!(!diagonal.axis_aligned() && !diagonal.on_boundary());
}

#[test]
fn liquid_faces_classify_by_plane() {
    assert_eq!(horizontal(WATER, 56).liquid(), Some(Liquid::Water));
    assert_eq!(horizontal(WATER, 56).liquid_dir(), Some((0, 1, 0)));
    assert_eq!(horizontal(WATER, 0).liquid_dir(), Some((0, -1, 0)));
    assert_eq!(side(WATER, (0, 0, -1)).liquid_dir(), Some((0, 0, -1)));
    assert_eq!(horizontal("minecraft:block/lava_flow", 56).liquid(), Some(Liquid::Lava));
    assert_eq!(horizontal(STONE, 56).liquid(), None);
}

#[test]
fn random_texture_variants_intern_equal() {
    assert_eq!(Tex::intern("minecraft:block/fire_1"), Tex::intern("minecraft:block/fire_0"));
    assert_ne!(Tex::intern("minecraft:block/fire_0"), Tex::intern(STONE));
}

#[test]
fn normalized_undoes_xz_offset_only() {
    let base = cross("minecraft:block/short_grass", 0, 0);
    assert_eq!(normalized(&cross("minecraft:block/short_grass", 5, -3)), normalized(&base));
    assert_eq!(normalized(&base), base, "a centred model is its own normal form");
    let lifted: Vec<FaceKey> =
        base.iter().map(|k| key("minecraft:block/short_grass", k.verts.map(|v| [v[0], v[1] + 1, v[2]]))).collect();
    assert_ne!(normalized(&lifted), normalized(&base));
}

#[test]
fn corner_aligned_and_close() {
    let a = corner_aligned(&cross("minecraft:block/fern", 5, -3));
    assert_eq!(a, corner_aligned(&cross("minecraft:block/fern", 0, 0)));
    assert!(a.iter().flat_map(|k| k.verts).any(|v| v[0] == 0 && v[2] == 0));

    let k = side(STONE, (1, 0, 0));
    let mut j = k;
    j.verts[0][1] += 1;
    assert!(close(&k, &j));
    j.verts[0][1] += 1;
    assert!(!close(&k, &j));
    assert!(!close(&k, &side("minecraft:block/dirt", (1, 0, 0))));
}

#[test]
fn world_faces_pair_triangles_and_add_tile_origin() {
    let quad = [[1.0, 65.0, 1.0], [2.0, 65.0, 1.0], [2.0, 65.0, 2.0], [1.0, 65.0, 2.0]];
    let lone = [[0.0, 70.0, 0.0], [1.0, 70.0, 0.0], [1.0, 70.0, 1.0]];
    let mut b = TileBuilder::default();
    b.quad(0, quad, [0, 127, 0]).triangle(9, lone, [0, 127, 0]);
    b.0.blocklight.fill(-1);
    let textures = texture_ids(&[STONE.into()]);
    let faces = world_faces(&b.0, [32, -16], &textures);
    assert_eq!(faces.len(), 2);

    let q = &faces[0];
    assert_eq!(q.verts, [[33.0, 65.0, -15.0], [34.0, 65.0, -15.0], [34.0, 65.0, -14.0], [33.0, 65.0, -14.0]]);
    assert_eq!(q.uv[3], [1.0, 1.0]);
    assert_eq!(q.texture, textures[0]);
    assert_eq!(q.normal, [0.0, 1.0, 0.0]);
    assert_eq!(q.blocklight, 0, "negative light clamps to 0");
    assert_eq!(q.owner(), (33, 64, -15));

    let degenerate = &faces[1];
    assert_eq!(degenerate.verts[3], degenerate.verts[2], "unpaired triangle repeats its last corner");
    assert_eq!(degenerate.texture, Tex::intern("?"), "material outside textures.json");
}

#[test]
fn cell_faces_track_tint_and_light() {
    let cell = (0, 0, 0);
    let mut c = CellFaces::default();
    c.push(&world_side(cell, UP, "minecraft:block/grass_block_top", [100, 200, 50], 7), cell);
    c.push(&world_side(cell, (1, 0, 0), "minecraft:block/grass_block_side", [255; 3], 3), cell);
    c.push(&world_side(cell, (-1, 0, 0), "minecraft:block/grass_block_side", [255; 3], 9), cell);
    assert_eq!(c.tint(), Some([100, 200, 50]));
    assert_eq!(c.light(), 3);
    assert_eq!((c.keys.len(), c.uvs.len()), (3, 3));

    let mut wet = CellFaces::default();
    wet.push(&world_side(cell, UP, WATER, [10, 20, 200], 0), cell);
    assert_eq!(wet.tint(), None, "water tint is biome noise");
    assert!(wet.uvs.is_empty());
    assert_eq!((wet.keys.len(), wet.light()), (1, 0));

    let mut other = CellFaces::default();
    other.push(&world_side(cell, (0, -1, 0), "minecraft:block/dirt", [200, 100, 150], 1), cell);
    c.merge(wet);
    c.merge(other);
    assert_eq!(c.tint(), Some([150, 150, 100]));
    assert_eq!(c.light(), 1);
    assert_eq!((c.keys.len(), c.uvs.len()), (5, 4));
}

#[test]
fn faces_group_by_owner() {
    let faces = [
        world_side((0, 0, 0), UP, STONE, [255; 3], 0),
        world_side((0, 0, 0), (1, 0, 0), STONE, [255; 3], 0),
        world_side((1, 0, 0), (-1, 0, 0), STONE, [255; 3], 0),
    ];
    let cells = faces_by_cell(&faces);
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[&(0, 0, 0)].keys.len(), 2);
    assert_eq!(cells[&(1, 0, 0)].keys, [side(STONE, (-1, 0, 0))]);
}
