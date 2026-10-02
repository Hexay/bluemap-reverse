use super::*;

const COL: Column = (0, 0);

/// Overworld column with blocks observed at `observed` ys and the given evidence.
fn column_gaps(observed: &[i32], ce: ColumnEvidence) -> Vec<(i32, i32, Fill)> {
    let bounds = Bounds { columns: vec![COL], profile: Profile::for_dimension("minecraft:overworld") };
    let observed = FxHashMap::from_iter([(COL, observed.to_vec())]);
    let evidence = FxHashMap::from_iter([(COL, ce)]);
    gaps(&observed, &evidence, &bounds).into_iter().map(|g| (g.ylo, g.yhi, g.fill)).collect()
}

#[test]
fn air_gap_under_the_cave_cutoff_turns_solid_below_its_evidence() {
    // seabed block at 47, open evidence at 46 (its bottom face was drawn), nothing below
    let ce = ColumnEvidence { open: vec![46], ..Default::default() };
    let got = column_gaps(&[47], ce);
    assert_eq!(got, [(48, 319, Fill::Air), (46, 46, Fill::Air), (-64, 45, Fill::Solid)]);
}

#[test]
fn lit_cave_between_drawn_floor_and_ceiling_stays_air() {
    let ce = ColumnEvidence { open: vec![40, 30], ..Default::default() };
    let got = column_gaps(&[41, 29], ce);
    assert!(got.contains(&(30, 40, Fill::Air)), "{got:?}");
}

#[test]
fn void_under_a_floating_block_stays_air() {
    // block at 71 over void (skyblock, the debug world): its bottom face drawn, nothing below in the
    // unculled range, so the space is open all the way down
    let ce = ColumnEvidence { open: vec![70], ..Default::default() };
    assert_eq!(column_gaps(&[71], ce)[1], (-64, 70, Fill::Air));
}

#[test]
fn water_over_rock_is_split_instead_of_outvoted() {
    // water drawn beside a flooded shaft at 44 and 0; its floor's missing faces say rock at -1, -2, -5
    let ce = ColumnEvidence {
        solid: vec![-1, -2, -5],
        liquid: vec![(44, Liquid::Water), (0, Liquid::Water)],
        ..Default::default()
    };
    let got = column_gaps(&[45], ce);
    assert_eq!(got[1..], [(0, 44, Fill::Liquid(Liquid::Water)), (-64, -1, Fill::Solid)]);
}

#[test]
fn several_layers_each_get_their_own_fill() {
    // flooded shaft side: water 44..0, rock at -1, an air pocket at -2, rock under it
    let ce = ColumnEvidence { solid: vec![-1], open: vec![-2], liquid: vec![(44, Liquid::Water), (0, Liquid::Water)] };
    let got = column_gaps(&[45], ce);
    assert_eq!(
        got[1..],
        [(0, 44, Fill::Liquid(Liquid::Water)), (-1, -1, Fill::Solid), (-2, -2, Fill::Air), (-64, -3, Fill::Solid)]
    );
}

#[test]
fn rock_between_runs_fills_the_unknown_cells() {
    // rock evidence at 40 and 39 over water at 30: rock down to just above the water
    let ce = ColumnEvidence { solid: vec![40, 39], liquid: vec![(30, Liquid::Water)], ..Default::default() };
    let got = column_gaps(&[45], ce);
    assert_eq!(got[1..], [(31, 44, Fill::Solid), (-64, 30, Fill::Liquid(Liquid::Water))]);
}

#[test]
fn liquid_evidence_names_the_liquid_of_an_open_cell() {
    // lava pocket above netherrack: the top cell is both open and lava, the rest only open
    let ce = ColumnEvidence { open: vec![18, 17, 13], liquid: vec![(18, Liquid::Lava)], ..Default::default() };
    assert_eq!(column_gaps(&[19, 12], ce)[1], (13, 18, Fill::Liquid(Liquid::Lava)));
}

#[test]
fn a_cell_with_solid_and_liquid_evidence_is_solid() {
    let ce = ColumnEvidence {
        solid: vec![30],
        liquid: vec![(40, Liquid::Water), (30, Liquid::Water)],
        ..Default::default()
    };
    let got = column_gaps(&[45], ce);
    assert_eq!(got[1..], [(40, 44, Fill::Liquid(Liquid::Water)), (-64, 39, Fill::Solid)]);
}

#[test]
fn liquid_and_solid_gaps_are_unchanged() {
    let water = ColumnEvidence { liquid: vec![(46, Liquid::Water)], ..Default::default() };
    assert_eq!(column_gaps(&[47], water)[1], (-64, 46, Fill::Liquid(Liquid::Water)));
    let rock = ColumnEvidence { solid: vec![46], ..Default::default() };
    assert_eq!(column_gaps(&[47], rock)[1], (-64, 46, Fill::Solid));
}
