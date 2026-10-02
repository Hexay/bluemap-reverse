use rustc_hash::{FxHashMap, FxHashSet};

use super::*;
use crate::test_util::*;

#[derive(Default)]
struct Scene {
    blocks: FxHashMap<Cell, usize>,
    solid_faces: FxHashMap<Cell, CellFaces>,
    liquid_faces: FxHashMap<Cell, Vec<FaceKey>>,
    liquids: FxHashMap<Cell, Liquid>,
}

impl Scene {
    fn block(&mut self, cell: Cell, entry: usize, culled: &[Cell]) -> &mut Self {
        let keys = cube(STONE).into_iter().filter(|k| !culled.contains(&k.boundary_dir().unwrap())).collect();
        self.blocks.insert(cell, entry);
        self.solid_faces.insert(cell, observed(keys));
        self
    }

    fn collect(&self, lib: &Library) -> Evidence {
        let o = Observed {
            blocks: &self.blocks,
            solid_faces: &self.solid_faces,
            liquid_faces: &self.liquid_faces,
            liquids: &self.liquids,
            ao: &[],
        };
        collect(lib, &o)
    }
}

fn set(cells: &[Cell]) -> FxHashSet<Cell> {
    cells.iter().copied().collect()
}

#[test]
fn missing_cullable_face_means_solid_neighbour_drawn_face_means_open() {
    let lib = library(vec![entry("minecraft:stone", cube(STONE))]);
    let mut s = Scene::default();
    s.block((0, 0, 0), 0, &[(1, 0, 0), (0, 0, 1)]).block((0, 0, 1), 0, &[(0, 0, -1)]);
    let ev = s.collect(&lib);
    // (0,0,1) is observed, so the face culled towards it says nothing new
    assert_eq!(ev.solid, set(&[(1, 0, 0)]));
    let open = [(-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, -1), (1, 0, 1), (-1, 0, 1), (0, 1, 1), (0, -1, 1), (0, 0, 2)];
    assert_eq!(ev.open, set(&open));
    assert!(ev.liquid.is_empty());
}

#[test]
fn a_drawn_face_overrules_a_missing_one() {
    let lib = library(vec![entry("minecraft:stone", cube(STONE))]);
    let mut s = Scene::default();
    // (0,0,0) misses its face towards (1,0,0); (2,0,0) draws its face towards it
    s.block((0, 0, 0), 0, &[(1, 0, 0)]).block((2, 0, 0), 0, &[]);
    let ev = s.collect(&lib);
    assert!(ev.open.contains(&(1, 0, 0)));
    assert!(ev.solid.is_empty());
}

#[test]
fn interior_faces_say_nothing_about_neighbours() {
    let candle = vec![side("minecraft:block/candle", (1, 0, 0)), horizontal("minecraft:block/candle", 32)];
    let lib = library(vec![entry("minecraft:candle", candle)]);
    let mut s = Scene::default();
    s.blocks.insert((0, 0, 0), 0);
    s.solid_faces.insert((0, 0, 0), observed(vec![]));
    let ev = s.collect(&lib);
    assert_eq!(ev.solid, set(&[(1, 0, 0)]));
    assert!(ev.open.is_empty());
}

#[test]
fn liquid_surface_opens_up_and_wets_the_rest() {
    let lib = library(vec![]);
    let mut s = Scene::default();
    s.liquids.insert((5, 0, 5), Liquid::Water);
    s.liquid_faces.insert((5, 0, 5), vec![horizontal(WATER, 56)]);
    let ev = s.collect(&lib);
    assert_eq!(ev.open, set(&[(5, 1, 5)]));
    let wet: FxHashSet<Cell> = ev.liquid.keys().copied().collect();
    assert_eq!(wet, set(&[(4, 0, 5), (6, 0, 5), (5, -1, 5), (5, 0, 4), (5, 0, 6)]));
    assert!(ev.liquid.values().all(|&l| l == Liquid::Water));
    assert!(ev.solid.is_empty());
}

#[test]
fn liquid_next_to_partial_block_wets_it_but_not_a_full_cube() {
    let slab = vec![horizontal(STONE, 32), side(STONE, (0, -1, 0))];
    let lib = library(vec![entry("minecraft:stone", cube(STONE)), entry("minecraft:smooth_stone_slab", slab)]);
    assert!(lib.entries[0].full_cube && !lib.entries[1].full_cube);
    let mut s = Scene::default();
    s.liquids.insert((0, 0, 0), Liquid::Lava);
    s.liquid_faces.insert((0, 0, 0), vec![horizontal("minecraft:block/lava_still", 56)]);
    s.block((1, 0, 0), 0, &[(-1, 0, 0)]);
    s.blocks.insert((-1, 0, 0), 1);
    s.solid_faces.insert((-1, 0, 0), observed(lib.entries[1].sig.clone()));
    let ev = s.collect(&lib);
    assert_eq!(ev.liquid.get(&(-1, 0, 0)), Some(&Liquid::Lava));
    assert!(!ev.liquid.contains_key(&(1, 0, 0)));
}
