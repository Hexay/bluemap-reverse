//! What drawn and missing faces say about unobserved neighbour cells (see docs/plan.md, fill):
//! - a matched block's cullable (non-liquid) face is missing → neighbour is a full opaque block (`solid`)
//! - a liquid face is missing → neighbour is the same liquid or a full block (`liquid`)
//! - any face drawn towards a neighbour → neighbour is air/liquid/transparent, not a full block (`open`)

use rustc_hash::{FxHashMap, FxHashSet};

use crate::face::{Cell, CellFaces, DIRS, FaceKey, Liquid, step};
use crate::library::Library;

#[derive(Default)]
pub struct Evidence {
    pub solid: FxHashSet<Cell>,
    pub liquid: FxHashMap<Cell, Liquid>,
    pub open: FxHashSet<Cell>,
}

pub struct Observed<'a> {
    /// Matched non-liquid blocks → library entry.
    pub blocks: &'a FxHashMap<Cell, usize>,
    /// Non-liquid faces per cell.
    pub solid_faces: &'a FxHashMap<Cell, CellFaces>,
    /// Liquid faces per cell (pure liquid cells and waterlogged blocks).
    pub liquid_faces: &'a FxHashMap<Cell, Vec<FaceKey>>,
    /// Cells showing nothing but liquid.
    pub liquids: &'a FxHashMap<Cell, Liquid>,
}

impl Observed<'_> {
    fn contains(&self, c: &Cell) -> bool {
        self.blocks.contains_key(c) || self.liquids.contains_key(c)
    }
}

pub fn collect(lib: &Library, o: &Observed) -> Evidence {
    let mut ev = Evidence::default();
    for (&cell, &entry) in o.blocks {
        let observed = o.solid_faces.get(&cell).map_or(&[][..], |c| &c.keys[..]);
        let mut remaining: Vec<&FaceKey> = observed.iter().collect();
        for key in &lib.entries[entry].sig {
            if let Some(i) = remaining.iter().position(|k| *k == key) {
                remaining.swap_remove(i);
            } else if let Some(n) = key.boundary_dir().map(|d| step(cell, d)).filter(|n| !o.contains(n)) {
                ev.solid.insert(n);
            }
        }
        for n in observed.iter().filter_map(|k| k.boundary_dir()).map(|d| step(cell, d)) {
            if !o.contains(&n) {
                ev.open.insert(n);
            }
        }
    }

    let liquid_cells = o
        .liquids
        .iter()
        .map(|(&c, &l)| (c, l))
        .chain(o.blocks.iter().filter_map(|(&c, &e)| lib.entries[e].liquid.map(|l| (c, l))))
        .chain(o.liquid_faces.iter().filter_map(|(&c, k)| k.first().and_then(FaceKey::liquid).map(|l| (c, l))));
    for (cell, kind) in liquid_cells {
        let faces = o.liquid_faces.get(&cell).map_or(&[][..], Vec::as_slice);
        for d in DIRS {
            let n = step(cell, d);
            if o.contains(&n) {
                continue;
            }
            if faces.iter().any(|k| k.liquid_dir() == Some(d)) {
                ev.open.insert(n);
            } else {
                ev.liquid.entry(n).or_insert(kind);
            }
        }
    }
    ev
}
