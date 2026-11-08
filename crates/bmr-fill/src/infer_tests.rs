use std::collections::BTreeMap;

use super::*;

const OVERWORLD: &str = "minecraft:overworld";
const NETHER: &str = "minecraft:the_nether";

/// `faces`: (y, triangles, cave triangles).
fn drawn(faces: &[(i32, u64, u64)]) -> DrawnHeights {
    DrawnHeights {
        faces: faces.iter().map(|&(y, n, _)| (y, n)).collect(),
        cave: faces.iter().filter(|f| f.2 > 0).map(|&(y, _, d)| (y, d)).collect::<BTreeMap<_, _>>(),
    }
}

fn settings(p: &Profile) -> (i32, i32, i32, Option<(i32, i32)>) {
    (p.min_y, p.max_y, p.cave_y, p.mask)
}

#[test]
fn default_overworld_keeps_the_defaults() {
    // faces deep down (a sunlit ravine, an ocean floor), cave faces only from the cut-off up
    let i =
        infer(Profile::for_dimension(OVERWORLD), &drawn(&[(-20, 50, 0), (55, 9, 4), (120, 900, 0)]), &Given::default());
    assert_eq!(settings(&i.profile), (-64, 319, 55, None));
    assert!(i.changed.is_empty());
    assert_eq!(i.summary, "y -64..319, caves hidden below y 55, no render mask");
}

#[test]
fn cave_faces_below_the_cut_off_mean_caves_are_drawn() {
    let faces = [(-64, 2, 2), (-50, 70, 70), (12, 30, 30), (64, 90, 0)];
    let i = infer(Profile::for_dimension(OVERWORLD), &drawn(&faces), &Given::default());
    assert_eq!(i.profile.cave_y, -50);
    assert_eq!(i.changed, ["cave-y"]);
}

#[test]
fn stray_faces_are_not_evidence() {
    let i = infer(Profile::for_dimension(OVERWORLD), &drawn(&[(-64, 2, 2), (12, 15, 15)]), &Given::default());
    assert_eq!(i.profile.cave_y, 55);
    let i = infer(Profile::for_dimension(NETHER), &drawn(&[(100, 4, 0), (127, 2, 0)]), &Given::default());
    assert_eq!(i.profile.mask, Some((90, 127)));
}

#[test]
fn cave_y_on_the_command_line_wins() {
    let given = Given { cave_y: Some(55), ..Given::default() };
    let i = infer(Profile::for_dimension(OVERWORLD), &drawn(&[(12, 30, 30)]), &given);
    assert_eq!(i.profile.cave_y, 55);
    assert!(i.changed.is_empty());
    assert_eq!(i.summary, "y -64..319, caves hidden below y 55 (given), no render mask");
}

#[test]
fn default_nether_keeps_its_roof_mask() {
    let i = infer(Profile::for_dimension(NETHER), &drawn(&[(1, 40, 40), (89, 30, 2), (130, 5, 0)]), &Given::default());
    assert_eq!(settings(&i.profile), (0, 255, -10_000, Some((90, 127))));
    assert!(i.changed.is_empty());
    assert_eq!(i.summary, "y 0..255, caves drawn, y 90..127 masked");
}

#[test]
fn a_drawn_roof_means_no_mask() {
    let i =
        infer(Profile::for_dimension(NETHER), &drawn(&[(89, 30, 0), (110, 80, 80), (127, 20, 0)]), &Given::default());
    assert_eq!(i.profile.mask, None);
    assert_eq!(i.changed, ["mask-y"]);
}

#[test]
fn faces_low_in_the_mask_shrink_it() {
    let i = infer(Profile::for_dimension(NETHER), &drawn(&[(95, 30, 0), (99, 18, 18), (120, 3, 0)]), &Given::default());
    assert_eq!(i.profile.mask, Some((100, 127)));
}

#[test]
fn given_mask_wins() {
    let given = Given { mask: Some(Some((90, 127))), ..Given::default() };
    let i = infer(Profile::for_dimension(NETHER), &drawn(&[(127, 20, 0)]), &given);
    assert_eq!(i.profile.mask, Some((90, 127)));
    assert!(i.summary.ends_with("y 90..127 masked (given)"));
}

#[test]
fn faces_beyond_the_build_height_extend_it_by_sections() {
    let i = infer(Profile::for_dimension(OVERWORLD), &drawn(&[(-70, 1, 0), (330, 1, 0)]), &Given::default());
    assert_eq!((i.profile.min_y, i.profile.max_y), (-80, 335));
    assert_eq!(i.changed, ["min-y", "max-y"]);
    let given = Given { min_y: Some(-64), max_y: Some(319), ..Given::default() };
    let i = infer(Profile::for_dimension(OVERWORLD), &drawn(&[(-70, 1, 0), (330, 1, 0)]), &given);
    assert_eq!((i.profile.min_y, i.profile.max_y), (-64, 319));
}

#[test]
fn empty_map_keeps_the_defaults() {
    let i = infer(Profile::for_dimension(NETHER), &DrawnHeights::default(), &Given::default());
    assert_eq!(settings(&i.profile), (0, 255, -10_000, Some((90, 127))));
}
