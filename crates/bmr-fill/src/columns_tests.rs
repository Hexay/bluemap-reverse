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
fn air_above_the_cutoff_is_kept() {
    // open evidence at 70 only: air down to the cutoff, rock under it
    let ce = ColumnEvidence { open: vec![70], ..Default::default() };
    let got = column_gaps(&[71], ce);
    assert!(got.contains(&(55, 70, Fill::Air)) && got.contains(&(-64, 54, Fill::Solid)), "{got:?}");
}

#[test]
fn liquid_and_solid_gaps_are_unchanged() {
    let water = ColumnEvidence { liquid: vec![(46, Liquid::Water)], ..Default::default() };
    assert_eq!(column_gaps(&[47], water)[1], (-64, 46, Fill::Liquid(Liquid::Water)));
    let rock = ColumnEvidence { solid: vec![46], ..Default::default() };
    assert_eq!(column_gaps(&[47], rock)[1], (-64, 46, Fill::Solid));
}
